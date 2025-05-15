// TODO: reduce code duplication. for example, with codegen

#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.h"

// .rodata: globals tunable from user space
const volatile bool debug_enabled SEC(".rodata") = false;
const volatile u64 system_boot_ns SEC(".rodata") = 0;

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

static __always_inline u64
make_upid(u32 pid, u64 start_ns)
{
	/* combine low 24 bits from pid, with 40 bits from start_ns */
	const u64 PID_MASK = 0x00FFFFFFULL;				/* 24 ones */
	const u64 TIME_MASK = 0x000FFFFFFFFFFULL; /* 40 ones */
	return ((u64)(pid & PID_MASK) << 40) | (start_ns & TIME_MASK);
}

SEC("tracepoint/sched/sched_process_exec")
int handle__sched__sched_process_exec(struct trace_event_raw_sched_process_exec *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	// === Common fields shared by every event === //
	e->event_type = EVENT__SCHED__SCHED_PROCESS_EXEC;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	// === Variant fields unique to sched_process_exec === //
	struct mm_struct *mm;
	unsigned long arg_start, arg_end, arg_ptr;
	u32 i;

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

	debug_printk("sched/sched_process_exec detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tracepoint/sched/sched_process_exit")
int handle__sched__sched_process_exit(struct trace_event_raw_sched_process_template *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	/* === Common fields shared by every event === */
	e->event_type = EVENT__SCHED__SCHED_PROCESS_EXIT;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	debug_printk("sched/sched_process_exit detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tracepoint/syscalls/sys_enter_openat")
int handle__syscall__sys_enter_openat(struct trace_event_raw_sys_enter *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	return 0; // Temporary, work-in-progress

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	/* === Common fields shared by every event === */
	e->event_type = EVENT__SYSCALL__SYS_ENTER_OPENAT;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	// === Variant fields unique to syscalls/sys_enter_openat === //
	e->syscall__sys_enter_openat__payload.dfd = BPF_CORE_READ(ctx, args[0]);
	bpf_probe_read_user_str(e->syscall__sys_enter_openat__payload.filename, MAX_STR_LEN, (void *)BPF_CORE_READ(ctx, args[1]));
	e->syscall__sys_enter_openat__payload.flags = BPF_CORE_READ(ctx, args[2]);
	e->syscall__sys_enter_openat__payload.mode = BPF_CORE_READ(ctx, args[3]);

	debug_printk("syscalls/sys_enter_openat detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tracepoint/syscalls/sys_exit_openat")
int handle__syscall__sys_exit_openat(struct trace_event_raw_sys_exit *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	return 0; // Temporary, work-in-progress

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	/* === Common fields shared by every event === */
	e->event_type = EVENT__SYSCALL__SYS_EXIT_OPENAT;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	// === Variant fields unique to syscalls/sys_exit_openat === //
	e->syscall__sys_exit_openat__payload.fd = ctx->ret;

	debug_printk("syscalls/sys_exit_openat detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

// // libbpf rewrites at compile-time; more portable than it appears at a glance
// struct trace_event_raw_oom_kill
// {
// 	__u16 common_type;
// 	__u8 common_flags;
// 	__u8 common_preempt_count;
// 	__s32 common_pid;

// 	__s32 pid;					 /* victim thread id   */
// 	__s32 tgid;					 /* victim tgid        */
// 	__s32 oom_score_adj; /* from /proc/...     */
// } __attribute__((preserve_access_index));

// struct trace_event_raw_oom_kill_process
// {
// 	__u16 common_type;
// 	__u8 common_flags;
// 	__u8 common_preempt_count;
// 	__s32 common_pid;

// 	__s32 pid;
// 	__s32 tgid;
// 	__s32 oom_score_adj;
// } __attribute__((preserve_access_index));

SEC("tracepoint/oom/oom_kill")
int handle__oom__oom_kill(struct trace_event_raw_oom_kill *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	return 0; // Temporary, work-in-progress

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	/* === Common fields shared by every event === */
	e->event_type = EVENT__OOM__OOM_KILL;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	// === Variant fields unique to oom/oom_kill === //
	e->oom__oom_kill__payload.oom_score_adj = BPF_CORE_READ(ctx, oom_score_adj);

	debug_printk("oom/oom_kill detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tracepoint/oom/oom_kill_process")
int handle__oom__oom_kill_process(struct trace_event_raw_oom_kill_process *ctx)
{
	u64 id = bpf_get_current_pid_tgid();
	u32 pid = id >> 32;
	u32 tid = (u32)id;

	// Ignore threads, report only the root process
	// todo: handle multi-threaded processes
	if (pid != tid)
		return 0;

	// todo: BPF_RB_NO_WAKEUP (perf)
	struct event *e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *parent = BPF_CORE_READ(task, parent);

	/* === Common fields shared by every event === */
	e->event_type = EVENT__OOM__OOM_KILL_PROCESS;
	e->timestamp_ns = bpf_ktime_get_ns() + system_boot_ns;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(parent, tgid);

	// Unique Process IDs (handles pid reuse)
	u64 start_ns = BPF_CORE_READ(task, start_time);
	u64 pstart_ns = BPF_CORE_READ(parent, start_time);
	e->upid = make_upid(e->pid, start_ns);
	e->uppid = make_upid(e->ppid, pstart_ns);

	// === Variant fields unique to oom/oom_kill_process === //
	e->oom__oom_kill_process__payload.victim_pid = BPF_CORE_READ(ctx, pid); // a.k.a victim_pid on old trees
	e->oom__oom_kill_process__payload.victim_tgid = BPF_CORE_READ(ctx, tgid);
	e->oom__oom_kill_process__payload.oom_score_adj = BPF_CORE_READ(ctx, oom_score_adj);

	debug_printk("oom/oom_kill_process detected\n");
	bpf_ringbuf_submit(e, 0);
	return 0;
}

// TODO: tracepoints that indicate memory pressure
// SEC("tracepoint/mm_vmscan_kswapd_wake")
// SEC("tracepoint/mm_vmscan_kswapd_sleep")
// SEC("tracepoint/mm_vmscan_direct_reclaim_begin")
// SEC("tracepoint/mm_vmscan_direct_reclaim_end")

// Licence, required to invoke GPL-restricted BPF functions
char LICENSE[] SEC("license") = "GPL";