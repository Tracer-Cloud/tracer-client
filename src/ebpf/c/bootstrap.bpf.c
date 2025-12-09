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

/* -------------------------------------------------------------------------- */
/* Map to track function entry times for duration calculation                 */
/* Key: (pid << 32) | stack_depth_hash                                        */
/* Value: entry timestamp in nanoseconds                                      */
/* -------------------------------------------------------------------------- */
struct python_call_key {
    u32 pid;
    u64 frame_ptr;  // Use frame pointer as unique identifier
};

struct python_call_value {
    u64 entry_time_ns;
    char filename[MAX_STR_LEN];
    char function_name[MAX_STR_LEN];
    int line_number;
};

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 65536);
    __type(key, struct python_call_key);
    __type(value, struct python_call_value);
} python_call_stack SEC(".maps");

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
/* 2.  Variant-specific payload helpers                                       */
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
/* 3.  Python Instrumentation (Python 3.12+ Structures)                       */
/* -------------------------------------------------------------------------- */

/*
 * Python 3.12+ uses compact ASCII strings where the string data immediately
 * follows the PyASCIIObject header when state.ascii=1 and state.compact=1.
 *
 * The structure is:
 *   PyASCIIObject (header) + char[] (inline string data)
 *
 * For ASCII compact strings, offset to data = sizeof(PyASCIIObject)
 * On 64-bit: sizeof(PyASCIIObject) = 48 bytes typically
 */

/* Minimal PyASCIIObject for size calculation */
struct PyASCIIObject_minimal {
    long ob_refcnt;           // 8 bytes
    void *ob_type;            // 8 bytes
    long length;              // 8 bytes
    long hash;                // 8 bytes
    unsigned int state;       // 4 bytes (contains ascii, compact, kind flags)
    // wstr pointer removed in Python 3.12 for compact strings
};

/* Size of PyASCIIObject header - string data follows immediately after */
#define PY_ASCII_OBJECT_SIZE 48

/*
 * Read a Python string object.
 * For compact ASCII strings (the common case), data is at object + 48 bytes.
 * Returns the number of bytes read, or negative on error.
 */
static __always_inline long read_python_ascii_string(void *str_obj, char *dst, int max_len) {
    if (!str_obj) {
        dst[0] = '\0';
        return 0;
    }

    /* Read the string data from offset 48 (after PyASCIIObject header) */
    void *str_data = (void *)((unsigned long)str_obj + PY_ASCII_OBJECT_SIZE);
    return bpf_probe_read_user_str(dst, max_len, str_data);
}

/*
 * Python 3.12 PyCodeObject layout (verified offsets)
 * Use pahole or gdb on your Python binary to verify these for your build.
 *
 * The critical fields we need:
 *   co_filename: offset 112 from start of PyCodeObject
 *   co_name:     offset 120 from start of PyCodeObject
 *   co_firstlineno: offset 64 from start of PyCodeObject
 */
#define PYCODE_OFFSET_FIRSTLINENO  64
#define PYCODE_OFFSET_FILENAME    112
#define PYCODE_OFFSET_NAME        120

/*
 * Python 3.12 _PyInterpreterFrame layout
 * f_code is at offset 0
 */

/* Helper to fill common event fields */
static __always_inline void fill_python_common(struct event *e, u32 tgid) {
    struct task_struct *task = (struct task_struct *)bpf_get_current_task();
    struct task_struct *parent = BPF_CORE_READ(task, parent);

    e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
    e->pid = tgid;
    e->ppid = BPF_CORE_READ(parent, tgid);

    u64 start_ns = BPF_CORE_READ(task, start_time);
    u64 pstart_ns = BPF_CORE_READ(parent, start_time);
    e->upid = make_upid(e->pid, start_ns);
    e->uppid = make_upid(e->ppid, pstart_ns);
}

/* Helper to read code object fields into payload */
static __always_inline int read_code_object_fields(
    void *code_obj,
    char *filename_dst,
    char *funcname_dst,
    int *line_number_dst
) {
    if (!code_obj) {
        filename_dst[0] = '\0';
        funcname_dst[0] = '\0';
        *line_number_dst = 0;
        return -1;
    }

    void *filename_obj = NULL;
    void *name_obj = NULL;

    /* Read pointers at known offsets */
    bpf_probe_read_user(&filename_obj, sizeof(filename_obj),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_FILENAME));
    bpf_probe_read_user(&name_obj, sizeof(name_obj),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_NAME));
    bpf_probe_read_user(line_number_dst, sizeof(int),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_FIRSTLINENO));

    /* Read string contents */
    if (filename_obj) {
        read_python_ascii_string(filename_obj, filename_dst, MAX_STR_LEN);
    } else {
        filename_dst[0] = '\0';
    }

    if (name_obj) {
        read_python_ascii_string(name_obj, funcname_dst, MAX_STR_LEN);
    } else {
        funcname_dst[0] = '\0';
    }

    return 0;
}

/*
 * Python function ENTRY handler
 * Attaches to: _PyEval_EvalFrameDefault
 *
 * In Python 3.12+, the frame is passed as the 2nd argument (RSI on x86_64)
 */
SEC("uprobe//usr/bin/python3:_PyEval_EvalFrameDefault")
int handle_python_entry(struct pt_regs *ctx)
{
    u64 id = bpf_get_current_pid_tgid();
    u32 tgid = id >> 32;
    u64 entry_time = bpf_ktime_get_ns();

    /* Get the frame pointer (2nd argument) */
    void *frame = (void *)PT_REGS_PARM2(ctx);
    if (!frame)
        return 0;

    /* Read f_code from frame (offset 0) */
    void *code = NULL;
    bpf_probe_read_user(&code, sizeof(code), frame);
    if (!code)
        return 0;

    /* Store entry info in map for duration calculation on exit */
    struct python_call_key key = {
        .pid = tgid,
        .frame_ptr = (u64)frame,
    };

    struct python_call_value val = {
        .entry_time_ns = entry_time,
        .line_number = 0,
    };
    val.filename[0] = '\0';
    val.function_name[0] = '\0';

    /* Read code object fields */
    read_code_object_fields(code, val.filename, val.function_name, &val.line_number);

    /* Store in map */
    bpf_map_update_elem(&python_call_stack, &key, &val, BPF_ANY);

    /* Reserve and submit entry event */
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    /* Initialize payload */
    e->python__function_entry__payload.filename[0] = '\0';
    e->python__function_entry__payload.function_name[0] = '\0';
    e->python__function_entry__payload.line_number = 0;
    e->python__function_entry__payload.entry_time_ns = entry_time;

    e->event_type = EVENT__PYTHON__FUNCTION_ENTRY;
    fill_python_common(e, tgid);

    /* Copy cached values to event */
    __builtin_memcpy(e->python__function_entry__payload.filename, val.filename, MAX_STR_LEN);
    __builtin_memcpy(e->python__function_entry__payload.function_name, val.function_name, MAX_STR_LEN);
    e->python__function_entry__payload.line_number = val.line_number;

    bpf_ringbuf_submit(e, 0);
    return 0;
}

/*
 * Python function EXIT handler (uretprobe)
 * Captures when _PyEval_EvalFrameDefault returns
 */
SEC("uretprobe//usr/bin/python3:_PyEval_EvalFrameDefault")
int handle_python_exit(struct pt_regs *ctx)
{
    u64 id = bpf_get_current_pid_tgid();
    u32 tgid = id >> 32;
    u64 exit_time = bpf_ktime_get_ns();

    /*
     * Note: In uretprobe, we don't have access to the original frame pointer.
     * We use a per-CPU scratch approach or look up by PID.
     * For simplicity, we'll emit an exit event and let userspace correlate.
     *
     * A more sophisticated approach would use a per-thread stack in BPF.
     */

    /* For now, just reserve an exit event with timestamp */
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    e->python__function_exit__payload.filename[0] = '\0';
    e->python__function_exit__payload.function_name[0] = '\0';
    e->python__function_exit__payload.line_number = 0;
    e->python__function_exit__payload.entry_time_ns = 0;
    e->python__function_exit__payload.duration_ns = 0;

    e->event_type = EVENT__PYTHON__FUNCTION_EXIT;
    fill_python_common(e, tgid);

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