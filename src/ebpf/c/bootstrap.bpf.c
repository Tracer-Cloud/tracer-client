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
/* Per-thread call stack for tracking function entry/exit                     */
/* -------------------------------------------------------------------------- */

#define MAX_STACK_DEPTH 128

struct stack_entry {
    u64 entry_time_ns;
    char filename[MAX_STR_LEN];
    char function_name[MAX_STR_LEN];
    int line_number;
    int _pad;  // Padding for alignment
};

/* Per-thread stack - key is pid_tgid */
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __uint(max_entries, 1);
    __type(key, u32);
    __type(value, struct stack_entry);
} scratch_entry SEC(".maps");

/* Stack depth per thread */
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 16384);
    __type(key, u64);    // pid_tgid
    __type(value, u32);  // current depth
} stack_depth SEC(".maps");

/* The actual stack entries - key is (pid_tgid << 7) | depth */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 262144);  // Support many nested calls
    __type(key, u64);
    __type(value, struct stack_entry);
} stack_entries SEC(".maps");

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

/* Create a unique key for stack entry lookup */
static __always_inline u64 make_stack_key(u64 pid_tgid, u32 depth) {
    return (pid_tgid << 7) | (depth & 0x7F);
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
 * Python 3.12 string layout for compact ASCII strings.
 *
 * Your system's verification script found strings at offset 40.
 * This is the header size of PyASCIIObject on your Python 3.12.3 build.
 */
#define PY_ASCII_OBJECT_SIZE 40

/*
 * PyCodeObject field offsets for Python 3.12
 * Verified by your script output:
 *   co_filename pointer at offset 112
 *   co_name pointer at offset 120
 *   co_firstlineno at offset 68
 */
#define PYCODE_OFFSET_FIRSTLINENO  68
#define PYCODE_OFFSET_FILENAME    112
#define PYCODE_OFFSET_NAME        120

/*
 * Read a Python ASCII string object's data.
 */
static __always_inline long read_python_ascii_string(void *str_obj, char *dst, int max_len) {
    if (!str_obj) {
        dst[0] = '\0';
        return 0;
    }

    void *str_data = (void *)((unsigned long)str_obj + PY_ASCII_OBJECT_SIZE);
    return bpf_probe_read_user_str(dst, max_len, str_data);
}

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

/* Helper to read code object fields into stack entry */
static __always_inline int read_code_object_fields(
    void *code_obj,
    struct stack_entry *entry
) {
    if (!code_obj) {
        entry->filename[0] = '\0';
        entry->function_name[0] = '\0';
        entry->line_number = 0;
        return -1;
    }

    void *filename_obj = NULL;
    void *name_obj = NULL;

    /* Read pointers at known offsets */
    bpf_probe_read_user(&filename_obj, sizeof(filename_obj),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_FILENAME));
    bpf_probe_read_user(&name_obj, sizeof(name_obj),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_NAME));
    bpf_probe_read_user(&entry->line_number, sizeof(int),
                        (void *)((unsigned long)code_obj + PYCODE_OFFSET_FIRSTLINENO));

    /* Read string contents */
    if (filename_obj) {
        read_python_ascii_string(filename_obj, entry->filename, MAX_STR_LEN);
    } else {
        entry->filename[0] = '\0';
    }

    if (name_obj) {
        read_python_ascii_string(name_obj, entry->function_name, MAX_STR_LEN);
    } else {
        entry->function_name[0] = '\0';
    }

    return 0;
}

/*
 * Python function ENTRY handler
 */
SEC("uprobe//usr/bin/python3:_PyEval_EvalFrameDefault")
int handle_python_entry(struct pt_regs *ctx)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u32 tgid = pid_tgid >> 32;
    u64 entry_time = bpf_ktime_get_ns();

    /* Get the frame pointer (2nd argument on x86_64) */
    void *frame = (void *)PT_REGS_PARM2(ctx);
    if (!frame)
        return 0;

    /* Read f_code from frame (offset 0) */
    void *code = NULL;
    bpf_probe_read_user(&code, sizeof(code), frame);
    if (!code)
        return 0;

    /* Get scratch space for building the entry */
    u32 zero = 0;
    struct stack_entry *scratch = bpf_map_lookup_elem(&scratch_entry, &zero);
    if (!scratch)
        return 0;

    /* Initialize and fill the entry */
    scratch->entry_time_ns = entry_time;
    scratch->filename[0] = '\0';
    scratch->function_name[0] = '\0';
    scratch->line_number = 0;

    read_code_object_fields(code, scratch);

    /* Get current depth for this thread */
    u32 *depth_ptr = bpf_map_lookup_elem(&stack_depth, &pid_tgid);
    u32 depth = 0;
    if (depth_ptr) {
        depth = *depth_ptr;
    }

    /* Store entry in stack */
    u64 stack_key = make_stack_key(pid_tgid, depth);
    bpf_map_update_elem(&stack_entries, &stack_key, scratch, BPF_ANY);

    /* Increment depth */
    u32 new_depth = depth + 1;
    bpf_map_update_elem(&stack_depth, &pid_tgid, &new_depth, BPF_ANY);

    /* Reserve and submit entry event */
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    e->event_type = EVENT__PYTHON__FUNCTION_ENTRY;
    fill_python_common(e, tgid);

    /* Copy from scratch to event payload */
    e->python__function_entry__payload.entry_time_ns = scratch->entry_time_ns;
    e->python__function_entry__payload.line_number = scratch->line_number;
    __builtin_memcpy(e->python__function_entry__payload.filename, scratch->filename, MAX_STR_LEN);
    __builtin_memcpy(e->python__function_entry__payload.function_name, scratch->function_name, MAX_STR_LEN);

    bpf_ringbuf_submit(e, 0);
    return 0;
}

/*
 * Python function EXIT handler (uretprobe)
 */
SEC("uretprobe//usr/bin/python3:_PyEval_EvalFrameDefault")
int handle_python_exit(struct pt_regs *ctx)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u32 tgid = pid_tgid >> 32;
    u64 exit_time = bpf_ktime_get_ns();

    /* Get current depth for this thread */
    u32 *depth_ptr = bpf_map_lookup_elem(&stack_depth, &pid_tgid);
    if (!depth_ptr || *depth_ptr == 0)
        return 0;

    /* Decrement depth first (to get the index of the entry we're returning from) */
    u32 depth = *depth_ptr - 1;
    bpf_map_update_elem(&stack_depth, &pid_tgid, &depth, BPF_ANY);

    /* Look up the entry from stack */
    u64 stack_key = make_stack_key(pid_tgid, depth);
    struct stack_entry *entry = bpf_map_lookup_elem(&stack_entries, &stack_key);
    if (!entry)
        return 0;

    u64 duration_ns = exit_time - entry->entry_time_ns;

    /* Reserve and submit exit event */
    struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
    if (!e)
        return 0;

    e->event_type = EVENT__PYTHON__FUNCTION_EXIT;
    fill_python_common(e, tgid);

    /* Copy from stack entry to event payload */
    __builtin_memcpy(e->python__function_exit__payload.filename, entry->filename, MAX_STR_LEN);
    __builtin_memcpy(e->python__function_exit__payload.function_name, entry->function_name, MAX_STR_LEN);
    e->python__function_exit__payload.line_number = entry->line_number;
    e->python__function_exit__payload.entry_time_ns = entry->entry_time_ns;
    e->python__function_exit__payload.duration_ns = duration_ns;

    /* Clean up the entry from the map */
    bpf_map_delete_elem(&stack_entries, &stack_key);

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