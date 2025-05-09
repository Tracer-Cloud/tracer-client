#ifndef __BOOTSTRAP_H
#define __BOOTSTRAP_H

#define TASK_COMM_LEN 16
#define MAX_FILENAME_LEN 127
#define MAX_ARGS 8
#define MAX_ARG_LEN 64

struct event
{
	int pid;
	int ppid;
	unsigned exit_code;
	char comm[TASK_COMM_LEN];
	char file_name[MAX_FILENAME_LEN];
	bool exit_event;
	__u64 started_at;
	int argc;
	char argv[MAX_ARGS][MAX_ARG_LEN];
};

#endif /* __BOOTSTRAP_H */
