#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.h"

/* -------------------------------------------------------------------------- */
/* Fixes for Compilation Errors                                               */
/* -------------------------------------------------------------------------- */
struct trace_event_raw_vmscan_direct_reclaim_begin;

/* -------------------------------------------------------------------------- */
/* Initialisation-time tunables & common helpers                              */
/* -------------------------------------------------------------------------- */
const volatile bool debug_enabled SEC(".rodata") = false;
const volatile u64 system_boot_ns SEC(".rodata") = 0;

struct
{
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 8 * 1024 * 1024);
} rb SEC(".maps");

static __always_inline void debug_printk(const char *fmt)
{
  if (unlikely(debug_enabled))
    bpf_printk("%s", fmt);
}

static __always_inline u64 make_upid(u32 pid, u64 start_ns)
{
  const u64 PID_MASK = 0x00FFFFFFULL;
  const u64 TIME_MASK = 0x000FFFFFFFFFFULL;
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

static __always_inline void
fill_sched_process_exec(struct event *e, struct trace_event_raw_sched_process_exec *ctx) {
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct mm_struct *mm;
  unsigned long arg_start, arg_end, arg_ptr;
  u32 i;

  BPF_CORE_READ_STR_INTO(&e->sched__sched_process_exec__payload.comm, task, comm);

  e->sched__sched_process_exec__payload.argc = 0;
  mm = BPF_CORE_READ(task, mm);
  if (!mm) return;

  arg_start = BPF_CORE_READ(mm, arg_start);
  arg_end = BPF_CORE_READ(mm, arg_end);
  arg_ptr = arg_start;

  for (i = 0; i < MAX_ARR_LEN; i++) {
    if (unlikely(arg_ptr >= arg_end)) break;
    long n = bpf_probe_read_user_str(&e->sched__sched_process_exec__payload.argv[i], MAX_STR_LEN, (void *)arg_ptr);
    if (n <= 0) break;
    e->sched__sched_process_exec__payload.argc++;
    arg_ptr += n;
  }
}

static __always_inline void
fill_sched_process_exit(struct event *e, struct trace_event_raw_sched_process_template *ctx) {
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  int exit_code = BPF_CORE_READ(task, exit_code);
  int exit_signal = BPF_CORE_READ(task, exit_signal);
  e->sched__sched_process_exit__payload.status = exit_code ? exit_code : exit_signal;
}

static __always_inline void
fill_sys_enter_openat(struct event *e, struct trace_event_raw_sys_enter *ctx) {
  e->syscall__sys_enter_openat__payload.dfd = BPF_CORE_READ(ctx, args[0]);
  bpf_probe_read_user_str(e->syscall__sys_enter_openat__payload.filename, MAX_STR_LEN, (void *)BPF_CORE_READ(ctx, args[1]));
  e->syscall__sys_enter_openat__payload.flags = BPF_CORE_READ(ctx, args[2]);
  e->syscall__sys_enter_openat__payload.mode = BPF_CORE_READ(ctx, args[3]);
}

static __always_inline void fill_sys_exit_openat(struct event *e, struct trace_event_raw_sys_exit *ctx) { e->syscall__sys_exit_openat__payload.fd = ctx->ret; }
static __always_inline void fill_sys_enter_read(struct event *e, struct trace_event_raw_sys_enter *ctx) { e->syscall__sys_enter_read__payload.fd = BPF_CORE_READ(ctx, args[0]); e->syscall__sys_enter_read__payload.count = BPF_CORE_READ(ctx, args[1]); }
static __always_inline void fill_sys_enter_write(struct event *e, struct trace_event_raw_sys_enter *ctx) { e->syscall__sys_enter_write__payload.fd = BPF_CORE_READ(ctx, args[0]); e->syscall__sys_enter_write__payload.count = BPF_CORE_READ(ctx, args[1]); }
static __always_inline void fill_vmscan_mm_vmscan_direct_reclaim_begin(struct event *e, struct trace_event_raw_vmscan_direct_reclaim_begin *ctx) { (void)e; }
static __always_inline void fill_oom_mark_victim(struct event *e, struct trace_event_raw_mark_victim *ctx) { (void)e; }

/* -------------------------------------------------------------------------- */
/* 3.  Python Instrumentation (Python 3.12 Structures)                        */
/* -------------------------------------------------------------------------- */

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

// Helper to read python strings
static __always_inline long read_python_string(void *obj, char *dst, int max_len) {
    return bpf_probe_read_user_str(dst, max_len, (void *)((long)obj + sizeof(PyASCIIObject)));
}

// Python 3.12 PyCodeObject (Approximate layout)
typedef struct {
    PyObject ob_base;             // 0
    PyObject *co_consts;          // 16
    PyObject *co_names;           // 24
    PyObject *co_exceptiontable;  // 32
    int co_flags;                 // 40
    short co_warmup;              // 44
    short _co_linearray_entry_size; // 46
    int co_argcount;              // 48
    int co_posonlyargcount;       // 52
    int co_kwonlyargcount;        // 56
    int co_stacksize;             // 60
    int co_firstlineno;           // 64
    int co_nlocalsplus;           // 68
    int co_framesize;             // 72
    int co_nlocals;               // 76
    int co_ncellvars;             // 80
    int co_nfreevars;             // 84
    u32 co_version;               // 88
    PyObject *co_localsplusnames; // 96
    PyObject *co_localspluskinds; // 104
    PyObject *co_filename;        // 112  <-- We want this
    PyObject *co_name;            // 120  <-- We want this
    PyObject *co_qualname;        // 128
    PyObject *co_linetable;       // 136
} PyCodeObject_312;

// Python 3.12 uses _PyInterpreterFrame, not PyFrameObject
typedef struct {
    PyCodeObject_312 *f_code; /* Strong reference */
    struct _PyInterpreterFrame *previous;
    PyObject *f_funcobj;
    PyObject *f_globals;
    PyObject *f_builtins;
    PyObject *f_locals;
} _PyInterpreterFrame;

// ATTACH TO: /usr/bin/python3 : _PyEval_EvalFrameDefault
// Note the leading underscore for Python 3.12!
SEC("uprobe//usr/bin/python3:_PyEval_EvalFrameDefault")
int handle_python_entry(struct pt_regs *ctx)
{
    u64 id = bpf_get_current_pid_tgid();
    u32 tgid = id >> 32;

    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    /* * ----------------------------------------------------------------------
     * FIX: Initialize payload fields manually
     * ----------------------------------------------------------------------
     * We cannot use memset() in BPF without compiler errors on some kernels.
     * We MUST set the first byte of strings to 0 so that if the probe fails
     * or fields are missing, the consumer sees a valid empty string ("")
     * instead of uninitialized memory (garbage) which causes UTF-8 errors.
     */
    e->python__function_entry__payload.function_name[0] = '\0';
    e->python__function_entry__payload.filename[0] = '\0';
    e->python__function_entry__payload.line_number = 0;

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

    // Python 3.12 Argument 2 (RSI) is the frame pointer
    _PyInterpreterFrame *frame = (_PyInterpreterFrame *)PT_REGS_PARM2(ctx);

    PyCodeObject_312 *code = 0;
    if (frame) {
        bpf_probe_read_user(&code, sizeof(code), &frame->f_code);
    }

    if (code) {
        void *name_obj = 0;
        void *filename_obj = 0;

        // Read pointers using the Python 3.12 layout
        bpf_probe_read_user(&name_obj, sizeof(name_obj), &code->co_name);
        bpf_probe_read_user(&filename_obj, sizeof(filename_obj), &code->co_filename);

        // Read the actual strings if the objects exist
        if (name_obj)
            read_python_string(name_obj, e->python__function_entry__payload.function_name, MAX_STR_LEN);

        if (filename_obj)
            read_python_string(filename_obj, e->python__function_entry__payload.filename, MAX_STR_LEN);

        bpf_probe_read_user(&e->python__function_entry__payload.line_number, sizeof(int), &code->co_firstlineno);
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
    u32 tgid = id >> 32;                                                          \
    u32 pid = (u32)id;                                                            \
    if (EVENT__##name != EVENT__SCHED__SCHED_PROCESS_EXIT && tgid != pid)        \
      return 0;                                                                   \
    if (EVENT__##name == EVENT__SCHED__SCHED_PROCESS_EXIT && tgid != pid)        \
      return 0;                                                                   \
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);                    \
    if (!e) return 0;                                                             \
    /* --- REMOVED: __builtin_memset(e, 0, sizeof(*e)); --- */                    \
    struct task_struct *task = (struct task_struct *)bpf_get_current_task();      \
    struct task_struct *parent = BPF_CORE_READ(task, parent);                     \
    e->event_type = EVENT__##name;                                                \
    e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;                        \
    e->pid = tgid;                                                                \
    e->ppid = BPF_CORE_READ(parent, tgid);                                        \
    u64 start_ns = BPF_CORE_READ(task, start_time);                               \
    u64 pstart_ns = BPF_CORE_READ(parent, start_time);                            \
    e->upid = make_upid(e->pid, start_ns);                                        \
    e->uppid = make_upid(e->ppid, pstart_ns);                                     \
    fill_fn(e, ctx);                                                              \
    bpf_ringbuf_submit(e, 0);                                                     \
    return 0;                                                                     \
  }

EVENT_LIST(HANDLER_DECL)
#undef HANDLER_DECL

char LICENSE[] SEC("license") = "GPL";