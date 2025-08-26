#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.h"

/* -------------------------------------------------------------------------- */
/*               Initialisation-time tunables & common helpers                */
/* -------------------------------------------------------------------------- */

// .rodata: globals tunable from user space
const volatile bool debug_enabled SEC(".rodata") = false;
const volatile u64 system_boot_ns SEC(".rodata") = 0;
const volatile char keys[MAX_KEYS][KEY_MAX_LEN] = {
    "TRACER_TRACE_ID=",
    /* add more (up to MAX_KEYS) */
};
const volatile int key_lens[MAX_KEYS] = {
    16, // strlen("TRACER_TRACE_ID=")
    /* add more (up to MAX_KEYS) */
};

// Ring buffer interface to user-space reader (bootstrap.c)
struct
{
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 256 * 1024);
} rb SEC(".maps");

// Print in debug mode
static __always_inline void debug_printk(const char *fmt)
{
  if (unlikely(debug_enabled))
    bpf_printk("%s", fmt);
}

// PIDs are not sufficient to uniquely identify processes,
// because of PID reuse, so we combine with process start time
static __always_inline u64 make_upid(u32 pid, u64 start_ns)
{
  const u64 PID_MASK = 0x00FFFFFFULL;       /* 24 ones */
  const u64 TIME_MASK = 0x000FFFFFFFFFFULL; /* 40 ones */
  return ((u64)(pid & PID_MASK) << 40) | (start_ns & TIME_MASK);
}

static __always_inline int startswith(const char *s, const char *p, int plen)
{
#pragma clang loop unroll(disable)
  for (int i = 0; i < plen; i++)
  {
    if (s[i] != p[i])
      return 0;
    if (!p[i])
      break;
  }
  return 1;
}

/* Tries to match the key with index `idx` against `str` - if they match, the value of the
 * environment variable is stored in the event payload. Returns `1` if a match was found,
 * `0` otherwise.
 */
static __always_inline int store_env_val(struct event *e, int idx, char *str, int str_len)
{
  // already found?
  if (e->sched__sched_process_exec__payload.env_found_mask & (1u << idx))
    return 0;

  const int key_len = key_lens[idx];
  if (str_len < key_len)
    return 0;
  if (!startswith(str, keys[idx], key_len))
    return 0;

  const char *val = str + key_len;

#pragma clang loop unroll(disable)
  for (int b = 0; b < VAL_MAX_LEN - 1; b++)
  {
    char c = val[b];
    e->sched__sched_process_exec__payload.env_values[idx][b] = c;
    if (c == '\0')
      break;
  }
  // ensure NUL
  e->sched__sched_process_exec__payload.env_values[idx][VAL_MAX_LEN - 1] = '\0';

  e->sched__sched_process_exec__payload.env_found_mask |= (1u << idx);
  return 1;
}

/* -------------------------------------------------------------------------- */
/*                         1.  Event registration table                       */
/* -------------------------------------------------------------------------- */

#define EVENT_LIST(X)                                                                                          \
  X(SCHED__SCHED_PROCESS_EXEC, trace_event_raw_sched_process_exec,                                             \
    "tracepoint/sched/sched_process_exec", fill_sched_process_exec)                                            \
  X(SCHED__SCHED_PROCESS_EXIT, trace_event_raw_sched_process_template,                                         \
    "tracepoint/sched/sched_process_exit", fill_sched_process_exit)                                            \
  /* keep syscalls commented to avoid self-trigger loops */                                                    \
  X(VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN, trace_event_raw_vmscan_direct_reclaim_begin,                       \
    "tracepoint/vmscan/mm_vmscan_direct_reclaim_begin", fill_vmscan_mm_vmscan_direct_reclaim_begin)            \
  X(OOM__MARK_VICTIM, trace_event_raw_mark_victim,                                                             \
    "tracepoint/oom/mark_victim", fill_oom_mark_victim)

/* -------------------------------------------------------------------------- */
/*                    2.  Variant-specific payload helpers                    */
/* -------------------------------------------------------------------------- */

// Process launched successfully
static __always_inline void
fill_sched_process_exec(struct event *e,
                        struct trace_event_raw_sched_process_exec *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct mm_struct *mm;

  // comm
  BPF_CORE_READ_STR_INTO(&e->sched__sched_process_exec__payload.comm, task, comm);

  // IMPORTANT: Keep argv empty to reduce insn count & avoid -E2BIG.
  e->sched__sched_process_exec__payload.argc = 0;

  mm = BPF_CORE_READ(task, mm);
  if (!mm)
    return;

  // ---- Atomically read env_start/env_end
  struct {
    unsigned long start;
    unsigned long end;
  } env;

  if (bpf_core_read(&env, sizeof(env), &mm->env_start) < 0)
    return;
  if (env.end <= env.start)
    return;

  // ---- Walk env block: NUL-terminated strings
  unsigned long p = env.start;
  int scanned_bytes = 0;
  int found = 0;

#pragma clang loop unroll(disable)
  for (int i = 0; i < MAX_ENV_STRS; i++)
  {
    if (p >= env.end)
      break;
    if (scanned_bytes >= MAX_SCAN_BYTES)
      break;

    // Use a reasonably large scratch buffer; MAX_STR_LEN typically ~256/512+
    char str[MAX_STR_LEN];
    long bytes_remaining = env.end - p;
    long read_len = bytes_remaining < (long)sizeof(str) ? bytes_remaining : (long)sizeof(str);

    long n = bpf_probe_read_user_str(str, read_len, (void *)p);
    if (n <= 1) {
      // invalid/empty → advance by 1 to avoid stalling
      p += 1;
      continue;
    }
    if (n == read_len) {
      // truncated → we can’t trust content nor alignment of next string; advance by n
      p += n;
      continue;
    }

    p += n;              // includes '\0'
    scanned_bytes += (int)n;

    // Only one key (index 0). If you add more, add more branches (don’t loop over MAX_KEYS).
    if (store_env_val(e, 0, str, (int)n))
      found++;

    if (found >= MAX_KEYS)
      break;
  }
}

// Process exited
static __always_inline void
fill_sched_process_exit(struct event *e,
                        struct trace_event_raw_sched_process_template *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  e->sched__sched_process_exit__payload.status = BPF_CORE_READ(task, exit_code);
}

// File open request started
static __always_inline void
fill_sys_enter_openat(struct event *e,
                      struct trace_event_raw_sys_enter *ctx)
{
  e->syscall__sys_enter_openat__payload.dfd = BPF_CORE_READ(ctx, args[0]);
  bpf_probe_read_user_str(e->syscall__sys_enter_openat__payload.filename,
                          MAX_STR_LEN, (void *)BPF_CORE_READ(ctx, args[1]));
  e->syscall__sys_enter_openat__payload.flags = BPF_CORE_READ(ctx, args[2]);
  e->syscall__sys_enter_openat__payload.mode = BPF_CORE_READ(ctx, args[3]);
}

// File open request successful
static __always_inline void
fill_sys_exit_openat(struct event *e,
                     struct trace_event_raw_sys_exit *ctx)
{
  e->syscall__sys_exit_openat__payload.fd = ctx->ret;
}

// File read
static __always_inline void
fill_sys_enter_read(struct event *e,
                    struct trace_event_raw_sys_enter *ctx)
{
  e->syscall__sys_enter_read__payload.fd = BPF_CORE_READ(ctx, args[0]);
  e->syscall__sys_enter_read__payload.count = BPF_CORE_READ(ctx, args[1]);
}

// File write
static __always_inline void
fill_sys_enter_write(struct event *e,
                     struct trace_event_raw_sys_enter *ctx)
{
  e->syscall__sys_enter_write__payload.fd = BPF_CORE_READ(ctx, args[0]);
  e->syscall__sys_enter_write__payload.count = BPF_CORE_READ(ctx, args[1]);
}

// Memory reclaim event
static __always_inline void
fill_vmscan_mm_vmscan_direct_reclaim_begin(struct event *e,
                                           struct trace_event_raw_vmscan_direct_reclaim_begin *ctx)
{
  (void)e;
}

// Memory stall event
static __always_inline void
fill_sched_psi_memstall_enter(struct event *e,
                              struct trace_event_raw_psi_memstall *ctx)
{
  (void)e;
}

// OOM mark victim event
static __always_inline void
fill_oom_mark_victim(struct event *e,
                     struct trace_event_raw_mark_victim *ctx __attribute__((unused)))
{
  (void)e;
}

/* -------------------------------------------------------------------------- */
/*                        3.  Generic handler generator                       */
/* -------------------------------------------------------------------------- */

#define HANDLER_DECL(name, ctx_t, sec, fill_fn)                                   \
  SEC(sec)                                                                        \
  int handle__##name(struct ctx_t *ctx)                                           \
  {                                                                               \
    /* --------------------------- common prologue --------------------------- */ \
    u64 id = bpf_get_current_pid_tgid();                                          \
    u32 pid = id >> 32;                                                           \
    u32 tid = (u32)id;                                                            \
                                                                                  \
    /* Ignore threads, only report the main thread */                             \
    if (pid != tid)                                                               \
      return 0;                                                                   \
                                                                                  \
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);                    \
    if (!e)                                                                       \
      return 0;                                                                   \
                                                                                  \
    struct task_struct *task = (struct task_struct *)bpf_get_current_task();      \
    struct task_struct *parent = BPF_CORE_READ(task, parent);                     \
                                                                                  \
    e->event_type = EVENT__##name;                                                \
    e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;                        \
    e->pid = pid;                                                                 \
    e->ppid = BPF_CORE_READ(parent, tgid);                                        \
                                                                                  \
    u64 start_ns = BPF_CORE_READ(task, start_time);                               \
    u64 pstart_ns = BPF_CORE_READ(parent, start_time);                            \
    e->upid = make_upid(e->pid, start_ns);                                        \
    e->uppid = make_upid(e->ppid, pstart_ns);                                     \
                                                                                  \
    /* ---------------------- variant-specific section ----------------------- */ \
    fill_fn(e, ctx);                                                              \
                                                                                  \
    bpf_ringbuf_submit(e, 0);                                                     \
    return 0;                                                                     \
  }

/* Instantiate one handler per EVENT_LIST entry */
EVENT_LIST(HANDLER_DECL)
#undef HANDLER_DECL

// Licence, required to invoke GPL-restricted BPF functions
char LICENSE[] SEC("license") = "GPL";
