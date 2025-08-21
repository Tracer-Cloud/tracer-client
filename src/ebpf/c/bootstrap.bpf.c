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
    16,
    /* add more (up to MAX_KEYS) */
};

// Ring buffer interface to user‑space reader (bootstrap.c)
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
  /* memcmp is verifier-friendly when plen is bounded */
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
  // check if we've already found the key
  if (e->sched__sched_process_exec__payload.env_found_mask & (1u << idx))
    return 0;
  // Ensure candidate string is at least key_len and matches prefix
  const int key_len = key_lens[idx];
  if (str_len < key_len)
    return 0;
  if (!startswith(str, keys[idx], key_len))
    return 0;
  // Copy value (portion after key)
  const char *val = str + key_len;
  // strncpy is not allowed; do bounded byte-wise copy
#pragma clang loop unroll(disable)
  for (int b = 0; b < VAL_MAX_LEN - 1; b++)
  {
    char c = val[b];
    e->sched__sched_process_exec__payload.env_values[idx][b] = c;
    if (c == '\0')
      break;
  }
  // make sure the value is null terminated
  e->sched__sched_process_exec__payload.env_values[idx][VAL_MAX_LEN - 1] = '\0';
  // mark the key as found
  e->sched__sched_process_exec__payload.env_found_mask |= (1u << idx);
  return 1;
}

/* -------------------------------------------------------------------------- */
/*                         1.  Event registration table                       */
/* -------------------------------------------------------------------------- */
//
// Each entry includes:
//
//   1. symbolic tail (matches EVENT__… names in bootstrap.h)
//   2. ctx struct type
//   3. SEC() section string
//   4. filler fn (collects fields specific to given tracepoint)
#define EVENT_LIST(X)                                                                                          \
  X(SCHED__SCHED_PROCESS_EXEC, trace_event_raw_sched_process_exec,                                             \
    "tracepoint/sched/sched_process_exec", fill_sched_process_exec)                                            \
  X(SCHED__SCHED_PROCESS_EXIT, trace_event_raw_sched_process_template,                                         \
    "tracepoint/sched/sched_process_exit", fill_sched_process_exit)                                            \
  /* TODO: cannot attach psi_memstall_enter */                                                                 \
  /* X(SCHED__PSI_MEMSTALL_ENTER, trace_event_raw_psi_memstall,                */                              \
  /*   "tracepoint/sched/psi_memstall_enter", fill_sched_psi_memstall_enter)   */                              \
                                                                                                               \
  /* TODO: collecting these events triggers them, causing indirect infinite loop. BPF_RB_NO_WAKEUP will fix */ \
  /*  X(SYSCALL__SYS_ENTER_OPENAT, trace_event_raw_sys_enter,                  */                              \
  /*    "tracepoint/syscalls/sys_enter_openat", fill_sys_enter_openat)         */                              \
  /*  X(SYSCALL__SYS_EXIT_OPENAT, trace_event_raw_sys_exit,                    */                              \
  /*    "tracepoint/syscalls/sys_exit_openat", fill_sys_exit_openat)           */                              \
  /*  X(SYSCALL__SYS_ENTER_READ, trace_event_raw_sys_enter,                    */                              \
  /*    "tracepoint/syscalls/sys_enter_read", fill_sys_enter_read)             */                              \
  /*  X(SYSCALL__SYS_ENTER_WRITE, trace_event_raw_sys_enter,                   */                              \
  /*    "tracepoint/syscalls/sys_enter_write", fill_sys_enter_write)           */                              \
                                                                                                               \
  X(VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN, trace_event_raw_vmscan_direct_reclaim_begin,                       \
    "tracepoint/vmscan/mm_vmscan_direct_reclaim_begin", fill_vmscan_mm_vmscan_direct_reclaim_begin)            \
                                                                                                               \
  X(OOM__MARK_VICTIM, trace_event_raw_mark_victim,                                                             \
    "tracepoint/oom/mark_victim", fill_oom_mark_victim)

/* -------------------------------------------------------------------------- */
/*                    2.  Variant‑specific payload helpers                    */
/* -------------------------------------------------------------------------- */

// Process launched successfully
static __always_inline void
fill_sched_process_exec(struct event *e,
                        struct trace_event_raw_sched_process_exec *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct mm_struct *mm;
  unsigned long arg_start, arg_end, arg_ptr, env_start, env_end;
  u32 i;

  BPF_CORE_READ_STR_INTO(&e->sched__sched_process_exec__payload.comm, task, comm);

  e->sched__sched_process_exec__payload.argc = 0;
  mm = BPF_CORE_READ(task, mm);
  if (!mm)
    return;

  arg_start = BPF_CORE_READ(mm, arg_start);
  arg_end = BPF_CORE_READ(mm, arg_end);
  arg_ptr = arg_start;

  for (i = 0; i < MAX_ARR_LEN; i++)
  {
    if (unlikely(arg_ptr >= arg_end))
      break;
    long n = bpf_probe_read_user_str(&e->sched__sched_process_exec__payload.argv[i],
                                     MAX_STR_LEN, (void *)arg_ptr);
    if (n <= 0)
      break;
    e->sched__sched_process_exec__payload.argc++;
    arg_ptr += n; // jump over NUL byte
  }

  // TODO: try this if env values are still getting corrupted:
  // Read env_start and env_end atomically to ensure consistency
  // struct env
  // {
  //  unsigned long start;
  //  unsigned long end;
  // };
  // // CO-RE relocate the offset of env_start within mm_struct.
  // long off = bpf_core_field_offset(struct mm_struct, env_start);
  // if (off < 0)
  //   break;
  // void *base = (void *)((char *)mm + off);
  // // Read start+end in one shot.
  // if (bpf_probe_read_kernel(&env, sizeof(env), base) < 0)
  //   return;
  // if (env.end < env.start)
  //   return;

  env_start = BPF_CORE_READ(mm, env_start);
  env_end = BPF_CORE_READ(mm, env_end);
  if (env_end <= env_start)
    return;

  /* Walk env block: NUL-terminated strings packed back-to-back */
  unsigned long p = env_start;
  int scanned_bytes = 0;
  int found = 0;

#pragma clang loop unroll(disable)
  for (int i = 0; i < MAX_ENV_STRS; i++)
  {
    if (p >= env_end)
      break;
    if (scanned_bytes >= MAX_SCAN_BYTES)
      break;
    char str[KEY_MAX_LEN + VAL_MAX_LEN]; /* room for key+value */
    long bytes_remaining = env_end - p;
    long read_len = bytes_remaining < sizeof(str) ? bytes_remaining : sizeof(str);
    long n = bpf_probe_read_user_str(str, read_len, (void *)p);
    p += (unsigned long)n;
    scanned_bytes += (int)n;
    if (n <= 1) /* invalid or empty string */
      continue;

    // NOTE: this currently works because we are only looking for a single key. Trying to look for
    // multiple keys in a loop will not work because the verifier will complain that program is too
    // complex. For each new key to look for, we will need to add a new branch here.
    if (store_env_val(e, 0, str, n))
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
  // TODO: get contents
  e->syscall__sys_enter_write__payload.fd = BPF_CORE_READ(ctx, args[0]);
  e->syscall__sys_enter_write__payload.count = BPF_CORE_READ(ctx, args[1]);
}

// Memory reclaim event
static __always_inline void
fill_vmscan_mm_vmscan_direct_reclaim_begin(struct event *e,
                                           struct trace_event_raw_vmscan_direct_reclaim_begin *ctx)
{
  // TODO: cannot read ctx->order, seems to be undefined on trace_event_raw_vmscan_direct_reclaim_begin
  (void)e;
  // e->vmscan__mm_vmscan_direct_reclaim_begin__payload.order = BPF_CORE_READ(ctx, order);
}

// Memory stall event
static __always_inline void
fill_sched_psi_memstall_enter(struct event *e,
                              struct trace_event_raw_psi_memstall *ctx)
{
  // TODO: cannot read ctx->type, seems to be undefined on trace_event_raw_psi_memstall
  (void)e;
  // e->sched__psi_memstall_enter__payload.type = BPF_CORE_READ(ctx, type);
}

// OOM mark victim event
static __always_inline void
fill_oom_mark_victim(struct event *e,
                     struct trace_event_raw_mark_victim *ctx __attribute__((unused)))
{
  // No additional fields to fill for OOM mark victim
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
    /* Ignore threads, report only the root process */                            \
    /* todo: handle multi-threaded processes */                                   \
    if (pid != tid)                                                               \
      return 0;                                                                   \
                                                                                  \
    /* todo: BPF_RB_NO_WAKEUP (perf) */                                           \
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);                    \
    if (!e)                                                                       \
      return 0;                                                                   \
                                                                                  \
    /* Fill fields common to every event */                                       \
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
    /* ---------------------- variant‑specific section ----------------------- */ \
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