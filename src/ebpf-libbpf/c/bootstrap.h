#ifndef __BOOTSTRAP_H
#define __BOOTSTRAP_H

#define TASK_COMM_LEN 16
#define MAX_FILENAME_LEN 32
#define MAX_ARGS 5
#define MAX_ARG_LEN 128

struct event
{
	int pid;
	int ppid;
	int event_type; // 0 for Start, 1 for Finish
	char comm[TASK_COMM_LEN];
	char file_name[MAX_FILENAME_LEN];
	char argv[MAX_ARGS][MAX_ARG_LEN];
	size_t len;
	__u64 time;
};

#endif /* __BOOTSTRAP_H */
