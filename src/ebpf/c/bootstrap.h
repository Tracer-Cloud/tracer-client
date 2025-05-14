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

struct event
{
	/* common fields */
	enum event_type event_type;
	u64 timestamp_ns;
	u32 pid;
	u32 ppid;

	/* variant payload */
	union
	{
		struct sched__sched_process_exec__payload sched__sched_process_exec__payload;
		struct sched__sched_process_exit__payload sched__sched_process_exit__payload;
	};
} __attribute__((packed));

#endif /* BOOTSTRAP_H */