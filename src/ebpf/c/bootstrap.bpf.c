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

// Ring buffer interface to user‑space reader (bootstrap.c)
struct
{
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 8 * 1024 * 1024);
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
  X(VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN, trace_event_raw_vmscan_direct_reclaim_begin,                       \
    "tracepoint/vmscan/mm_vmscan_direct_reclaim_begin", fill_vmscan_mm_vmscan_direct_reclaim_begin)            \
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
}

// Process exited
static __always_inline void
fill_sched_process_exit(struct event *e,
                        struct trace_event_raw_sched_process_template *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();

  // Read both exit_code and exit_signal
  int exit_code = BPF_CORE_READ(task, exit_code);
  int exit_signal = BPF_CORE_READ(task, exit_signal);

  // Combine them: typically exit_code contains the status
  // but exit_signal might have the signal if killed
  e->sched__sched_process_exit__payload.status = exit_code ? exit_code : exit_signal;

  // Debug: uncomment to see what values we're getting
  // if (debug_enabled)
  //   bpf_printk("EXIT: pid=%d exit_code=%d exit_signal=%d",
  //              BPF_CORE_READ(task, tgid), exit_code, exit_signal);
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
    u32 tgid = id >> 32;      /* thread-group id (the process id) */             \
    u32 pid = (u32)id;        /* actual kernel thread id (tid) */                \
                                                                                  \
    /* For non-exit events, ignore non-leader threads (only consider group leader) */ \
    if (EVENT__##name != EVENT__SCHED__SCHED_PROCESS_EXIT && tgid != pid)        \
      return 0;                                                                   \
                                                                                  \
    /* For EXIT, only report when the main thread (tgid == pid) exits */         \
    if (EVENT__##name == EVENT__SCHED__SCHED_PROCESS_EXIT && tgid != pid)        \
      return 0;                                                                   \
                                                                                  \
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
    /* store the process id (tgid) as the logical PID for events */              \
    e->pid = tgid;                                                                \
    e->ppid = BPF_CORE_READ(parent, tgid);                                        \
                                                                                  \
    /* Use the leader/start-time pairing that makes upid unique: */               \
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