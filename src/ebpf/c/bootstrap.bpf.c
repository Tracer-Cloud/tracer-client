#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.h"

/* -------------------------------------------------------------------------- */
/* Fixes for Compilation Errors                                               */
/* -------------------------------------------------------------------------- */

// FIX: Forward declare this struct to make it visible globally.
// This prevents the "incompatible pointer types" and visibility warnings.
struct trace_event_raw_vmscan_direct_reclaim_begin;

/* -------------------------------------------------------------------------- */
/* Initialisation-time tunables & common helpers                              */
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

/* -------------------------------------------------------------------------- */
/* 1.  Event registration table                                               */
/* -------------------------------------------------------------------------- */

#define EVENT_LIST(X)                                                                                          \
  X(SCHED__SCHED_PROCESS_EXEC, trace_event_raw_sched_process_exec,                                             \
    "tracepoint/sched/sched_process_exec", fill_sched_process_exec)                                            \
  X(SCHED__SCHED_PROCESS_EXIT, trace_event_raw_sched_process_template,                                         \
    "tracepoint/sched/sched_process_exit", fill_sched_process_exit)                                            \
  X(VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN, trace_event_raw_vmscan_direct_reclaim_begin,                       \
    "tracepoint/vmscan/mm_vmscan_direct_reclaim_begin", fill_vmscan_mm_vmscan_direct_reclaim_begin)            \
  X(OOM__MARK_VICTIM, trace_event_raw_mark_victim,                                                             \
    "tracepoint/oom/mark_victim", fill_oom_mark_victim)                                                        \
  X(SYSCALL__SYS_ENTER_OPENAT, trace_event_raw_sys_enter,                                                      \
    "tracepoint/syscalls/sys_enter_openat", fill_sys_enter_openat)

/* -------------------------------------------------------------------------- */
/* 2.  Variant‑specific payload helpers                                       */
/* -------------------------------------------------------------------------- */

// Process launched successfully
static __always_inline void
fill_sched_process_exec(struct event *e,
                        struct trace_event_raw_sched_process_exec *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct mm_struct *mm;
  unsigned long arg_start, arg_end, arg_ptr;
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

// OOM mark victim event
static __always_inline void
fill_oom_mark_victim(struct event *e,
                     struct trace_event_raw_mark_victim *ctx __attribute__((unused)))
{
  (void)e;
}

/* -------------------------------------------------------------------------- */
/* 3.  Python Instrumentation (Python 3.10 Structures)                        */
/* -------------------------------------------------------------------------- */

// FIX: Define wchar_t because vmlinux.h does not define standard C types.
typedef int wchar_t;

typedef struct {
    long ob_refcnt;
    void *ob_type;
} PyObject;

typedef struct {
    PyObject ob_base;
    long length;
    long hash;
    struct {
        unsigned int interned:2;
        unsigned int kind:3;
        unsigned int compact:1;
        unsigned int ascii:1;
        unsigned int ready:1;
        unsigned int :24;
    } state;
    wchar_t *wstr;
} PyASCIIObject;

typedef struct {
    PyASCIIObject _base;
    long utf8_length;
    char *utf8;
    long wstr_length;
} PyCompactUnicodeObject;

// Helper to read python strings
static __always_inline long read_python_string(void *obj, char *dst, int max_len) {
    // Note: This assumes compact ASCII strings (common for filenames/funcnames)
    // A robust implementation would check 'kind' and handle other encodings
    return bpf_probe_read_user_str(dst, max_len, (void *)((long)obj + sizeof(PyASCIIObject)));
}

typedef struct {
    PyObject ob_base;
    int co_argcount;
    int co_posonlyargcount;
    int co_kwonlyargcount;
    int co_nlocals;
    int co_stacksize;
    int co_flags;
    PyObject *co_code;
    PyObject *co_consts;
    PyObject *co_names;
    PyObject *co_varnames;
    PyObject *co_freevars;
    PyObject *co_cellvars;
    void *co_cell2arg;
    PyObject *co_filename;
    PyObject *co_name;
    int co_firstlineno;
} PyCodeObject;

typedef struct {
    PyObject ob_base;
    struct _frame *f_back;
    PyCodeObject *f_code;
    PyObject *f_builtins;
    PyObject *f_globals;
    PyObject *f_locals;
    PyObject **f_valuestack;
    PyObject *f_trace;
    int f_stackdepth;
    char f_trace_lines;
    char f_trace_opcodes;
    void *f_gen;
    int f_lasti;
    int f_lineno;
} PyFrameObject;

// Uprobe Handler for PyEval_EvalFrameDefault
// Note: "uprobe/python_eval" is a placeholder name. You must attach this
// via libbpf to the specific binary path (e.g. /usr/bin/python3).
SEC("uprobe/python_eval")
int handle_python_entry(struct pt_regs *ctx)
{
    u64 id = bpf_get_current_pid_tgid();
    u32 tgid = id >> 32;
    u32 pid = (u32)id;

    // Reserve ring buffer
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    // Fill common fields
    struct task_struct *task = (struct task_struct *)bpf_get_current_task();
    struct task_struct *parent = BPF_CORE_READ(task, parent);

    e->event_type = EVENT__PYTHON__FUNCTION_ENTRY;
    e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
    e->pid = tgid;
    e->ppid = BPF_CORE_READ(parent, tgid);

    u64 start_ns = BPF_CORE_READ(task, start_time);
    u64 pstart_ns = BPF_CORE_READ(parent, start_time);
    e->upid = make_upid(e->pid, start_ns);
    e->uppid = make_upid(e->ppid, pstart_ns);

    // Python specific payload filling
    // On x86_64, the first argument is passed in %rdi (PT_REGS_PARM1)
    PyFrameObject *frame = (PyFrameObject *)PT_REGS_PARM1(ctx);

    PyCodeObject *code = 0;
    if (frame) {
        bpf_probe_read_user(&code, sizeof(code), &frame->f_code);
    }

    if (code) {
        void *name_obj = 0;
        void *filename_obj = 0;

        // Read pointers to the string objects
        bpf_probe_read_user(&name_obj, sizeof(name_obj), &code->co_name);
        bpf_probe_read_user(&filename_obj, sizeof(filename_obj), &code->co_filename);

        // Read the actual strings
        if (name_obj)
            read_python_string(name_obj, e->python__function_entry__payload.function_name, MAX_STR_LEN);

        if (filename_obj)
            read_python_string(filename_obj, e->python__function_entry__payload.filename, MAX_STR_LEN);

        bpf_probe_read_user(&e->python__function_entry__payload.line_number, sizeof(int), &code->co_firstlineno);
    } else {
        // Fallback or empty if we couldn't read the code object
        e->python__function_entry__payload.function_name[0] = '\0';
        e->python__function_entry__payload.filename[0] = '\0';
        e->python__function_entry__payload.line_number = 0;
    }

    bpf_ringbuf_submit(e, 0);
    return 0;
}

/* -------------------------------------------------------------------------- */
/* 4.  Generic handler generator                                              */
/* -------------------------------------------------------------------------- */

#define HANDLER_DECL(name, ctx_t, sec, fill_fn)                                   \
  SEC(sec)                                                                        \
  int handle__##name(struct ctx_t *ctx)                                           \
  {                                                                               \
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
    fill_fn(e, ctx);                                                              \
                                                                                  \
    bpf_ringbuf_submit(e, 0);                                                     \
    return 0;                                                                     \
  }

EVENT_LIST(HANDLER_DECL)
#undef HANDLER_DECL

// Licence, required to invoke GPL-restricted BPF functions
char LICENSE[] SEC("license") = "GPL";