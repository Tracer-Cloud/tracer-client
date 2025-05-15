#ifndef BOOTSTRAP_H
#define BOOTSTRAP_H

#define TASK_COMM_LEN 16
#define MAX_ARR_LEN 16
#define MAX_STR_LEN 128

typedef unsigned long long u64;
typedef unsigned int u32;

enum event_type
{
	EVENT__SCHED__SCHED_PROCESS_EXEC = 0,
	EVENT__SCHED__SCHED_PROCESS_EXIT = 1,
	EVENT__SYSCALL__SYS_ENTER_OPENAT = 2,
	EVENT__SYSCALL__SYS_EXIT_OPENAT = 3,
	EVENT__OOM__OOM_KILL = 5,
	EVENT__OOM__OOM_KILL_PROCESS = 6,
	EVENT__MM_VMSCAN_KSWAPD_WAKE = 7,
	EVENT__MM_VMSCAN_KSWAPD_SLEEP = 8,
	EVENT__MM_VMSCAN_DIRECT_RECLAIM_BEGIN = 9,
	EVENT__MM_VMSCAN_DIRECT_RECLAIM_END = 10,
};

struct sched__sched_process_exec__payload
{
	char comm[TASK_COMM_LEN];
	u32 argc;
	char argv[MAX_ARR_LEN][MAX_STR_LEN];
};

struct sched__sched_process_exit__payload
{
};

struct syscall__sys_enter_openat__payload
{
	int dfd;
	char filename[MAX_STR_LEN];
	int flags;
	int mode;
};

struct syscall__sys_exit_openat__payload
{
	int fd;
};

struct oom__oom_kill__payload
{
	u32 victim_pid;
	u32 victim_tgid;
	u32 oom_score_adj;
};

struct oom__oom_kill_process__payload
{
	u32 victim_pid;
	u32 victim_tgid;
	u32 oom_score_adj;
};

struct mm_vmscan_kswapd_wake__payload
{
	u32 node_id;
};

struct mm_vmscan_kswapd_sleep__payload
{
	u32 node_id;
};

struct mm_vmscan_direct_reclaim_begin__payload
{
	u32 node_id;
	u32 order;
};

struct mm_vmscan_direct_reclaim_end__payload
{
	u32 node_id;
	u32 order;
	u32 reclaimed;
};

struct event
{
	/* common fields */
	enum event_type event_type;
	u64 timestamp_ns;
	u32 pid;
	u32 ppid;
	u64 upid;
	u64 uppid;

	/* variant payload */
	union
	{
		struct sched__sched_process_exec__payload sched__sched_process_exec__payload;
		struct sched__sched_process_exit__payload sched__sched_process_exit__payload;
		struct syscall__sys_enter_openat__payload syscall__sys_enter_openat__payload;
		struct syscall__sys_exit_openat__payload syscall__sys_exit_openat__payload;
		struct oom__oom_kill__payload oom__oom_kill__payload;
		struct oom__oom_kill_process__payload oom__oom_kill_process__payload;
		struct mm_vmscan_kswapd_wake__payload mm_vmscan_kswapd_wake__payload;
		struct mm_vmscan_kswapd_sleep__payload mm_vmscan_kswapd_sleep__payload;
		struct mm_vmscan_direct_reclaim_begin__payload mm_vmscan_direct_reclaim_begin__payload;
		struct mm_vmscan_direct_reclaim_end__payload mm_vmscan_direct_reclaim_end__payload;
	};
} __attribute__((packed));

#endif /* BOOTSTRAP_H */