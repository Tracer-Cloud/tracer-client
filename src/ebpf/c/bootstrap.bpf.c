#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.h"

// .rodata: globals tunable from user space
const volatile bool debug_enabled SEC(".rodata") = false;

// Ring buffer: interface for submitting events to userspace (bootstrap.c)
struct
{
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} rb SEC(".maps");

// Helpers
static __always_inline void
debug_printk(const char *fmt)
{
	if (unlikely(debug_enabled))
		bpf_printk("%s", fmt);
}

SEC("tracepoint/sched/sched_process_exec")
int handle_exec(struct trace_event_raw_sched_process_exec *ctx)
{
	struct event *e;
	struct task_struct *task;
	struct mm_struct *mm;
	unsigned long arg_start, arg_end, arg_ptr;
	u32 i;

	// todo: BPF_RB_NO_WAKEUP (perf);
	e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	// Common fields shared by every event
	task = (struct task_struct *)bpf_get_current_task();
	e->event_type = EVENT__SCHED__SCHED_PROCESS_EXEC;
	e->timestamp_ns = bpf_ktime_get_ns();
	e->pid = bpf_get_current_pid_tgid() >> 32;
	e->ppid = BPF_CORE_READ(task, real_parent, tgid);

	// Variant fields unique to sched_process_exec
	BPF_CORE_READ_STR_INTO(&e->sched__sched_process_exec__payload.comm, task, comm);

	e->sched__sched_process_exec__payload.argc = 0;
	mm = BPF_CORE_READ(task, mm);
	if (mm)
	{
		arg_start = BPF_CORE_READ(mm, arg_start);
		arg_end = BPF_CORE_READ(mm, arg_end);
		arg_ptr = arg_start;

		for (i = 0; i < MAX_ARR_LEN; i++)
		{
			if (unlikely(arg_ptr >= arg_end))
				break;

			long n = bpf_probe_read_user_str(&e->sched__sched_process_exec__payload.argv[i],
																			 MAX_STR_LEN,
																			 (void *)arg_ptr);
			if (n <= 0)
				break;

			e->sched__sched_process_exec__payload.argc++;
			arg_ptr += n; // Jump over NUL byte
		}
	}

	debug_printk("exec detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tracepoint/sched/sched_process_exit")
int handle_exit(struct trace_event_raw_sched_process_template *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	// Ignore threads, report only the final task exit
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();

	e->event_type = EVENT__SCHED__SCHED_PROCESS_EXIT;
	e->timestamp_ns = bpf_ktime_get_ns();
	e->pid = pid;
	e->ppid = BPF_CORE_READ(task, real_parent, tgid);

	debug_printk("exit detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

// Licence, required to invoke GPL-restricted BPF functions
char LICENSE[] SEC("license") = "GPL";