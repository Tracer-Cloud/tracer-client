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
	};
} __attribute__((packed));

#endif /* BOOTSTRAP_H */