#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include <bpf/bpf_core_read.h>
#include "bootstrap.h"

// Without this, we get runtime error "cannot call GPL-restricted function from non-GPL compatible program"
// This file (and only this file) has to be licensed as GPL
char LICENSE[] SEC("license") = "GPL";

struct
{
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, 256 * 1024);
} rb SEC(".maps");

SEC("tp/sched/sched_process_exec")
int handle_exec(struct trace_event_raw_sched_process_exec *ctx)
{
	struct task_struct *task;
	unsigned fname_off;
	struct event *e;
	pid_t pid;
	struct mm_struct *mm;
	unsigned long arg_start, arg_end;
	unsigned long arg_ptr;
	int i;

	/* reserve sample from BPF ringbuf */
	e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	/* fill out the sample with data */
	task = (struct task_struct *)bpf_get_current_task();
	pid = bpf_get_current_pid_tgid() >> 32;

	e->exit_event = false;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(task, real_parent, tgid);
	e->started_at = bpf_ktime_get_ns();
	BPF_CORE_READ_STR_INTO(&e->comm, task, comm);

	fname_off = ctx->__data_loc_filename & 0xFFFF;
	bpf_probe_read_str(&e->file_name, sizeof(e->file_name), (void *)ctx + fname_off);

	/* Extract argv from process memory */
	mm = BPF_CORE_READ(task, mm);
	if (!mm)
	{
		e->argc = 0;
		goto submit;
	}

	arg_start = BPF_CORE_READ(mm, arg_start);
	arg_end = BPF_CORE_READ(mm, arg_end);
	arg_ptr = arg_start;
	e->argc = 0;

/* Read up to MAX_ARGS arguments */
#pragma unroll
	for (i = 0; i < MAX_ARGS; i++)
	{
		if (arg_ptr >= arg_end)
		{
			break;
		}

		int len = bpf_probe_read_user_str(&e->argv[i], MAX_ARG_LEN, (const char *)arg_ptr);
		if (len <= 0)
		{
			break;
		}
		e->argc++;

		/* Move to the next string (len includes the null terminator) */
		arg_ptr += len;
	}

submit:
	/* successfully submit it to user-space for post-processing */
	bpf_ringbuf_submit(e, 0);
	return 0;
}

SEC("tp/sched/sched_process_exit")
int handle_exit(struct trace_event_raw_sched_process_template *ctx)
{
	struct task_struct *task;
	struct event *e;
	pid_t pid, tid;
	u64 id;

	/* get PID and TID of exiting thread/process */
	id = bpf_get_current_pid_tgid();
	pid = id >> 32;
	tid = (u32)id;

	/* ignore thread exits */
	if (pid != tid)
		return 0;

	/* reserve sample from BPF ringbuf */
	e = bpf_ringbuf_reserve(&rb, sizeof(*e), 0);
	if (!e)
		return 0;

	/* fill out the sample with data */
	task = (struct task_struct *)bpf_get_current_task();

	e->exit_event = true;
	e->pid = pid;
	e->ppid = BPF_CORE_READ(task, real_parent, tgid);
	e->exit_code = BPF_CORE_READ(task, exit_code) >> 8 & 0xff;
	e->started_at = bpf_ktime_get_ns();
	BPF_CORE_READ_STR_INTO(&e->comm, task, comm);
	e->argc = 0; /* No argv for exit events */

	/* send data to user-space for post-processing */
	bpf_ringbuf_submit(e, 0);
	return 0;
}
