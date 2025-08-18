// bootstrap.h
#ifndef BOOTSTRAP_H
#define BOOTSTRAP_H

#define TASK_COMM_LEN 16
#define MAX_ARR_LEN 16
#define MAX_STR_LEN 128

#define MAX_KEYS 1          /* how many env vars to capture */
#define KEY_MAX_LEN 16      /* max key name length incl '=' */
#define VAL_MAX_LEN 128     /* max captured value length (bytes) */
#define MAX_ENV_STRS 128    /* max env strings to scan */
#define MAX_SCAN_BYTES 4096 /* hard cap on bytes scanned */

typedef unsigned long long u64;
typedef unsigned int u32;

enum event_type
{
    EVENT__SCHED__SCHED_PROCESS_EXEC = 0,
    EVENT__SCHED__SCHED_PROCESS_EXIT = 1,
    EVENT__SCHED__PSI_MEMSTALL_ENTER = 16,

    EVENT__SYSCALL__SYS_ENTER_OPENAT = 1024,
    EVENT__SYSCALL__SYS_EXIT_OPENAT = 1025,
    EVENT__SYSCALL__SYS_ENTER_READ = 1026,
    EVENT__SYSCALL__SYS_EXIT_READ = 1027,
    EVENT__SYSCALL__SYS_ENTER_WRITE = 1028,
    EVENT__SYSCALL__SYS_EXIT_WRITE = 1029,

    EVENT__VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN = 2048,

    EVENT__OOM__MARK_VICTIM = 3072
};

struct sched__sched_process_exec__payload
{
    char comm[TASK_COMM_LEN];
    u32 argc;
    char argv[MAX_ARR_LEN][MAX_STR_LEN];
    u32 env_found_mask;                     // bit j == 1 => env_values[j] is populated
    char env_values[MAX_KEYS][VAL_MAX_LEN]; // Environment variables
};

struct sched__sched_process_exit__payload
{
    int status; // the status (see exit(3))
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

struct syscall__sys_enter_read__payload
{
    int fd;
    size_t count;
};

struct syscall__sys_enter_write__payload
{
    int fd;
    size_t count;
};

struct vmscan__mm_vmscan_direct_reclaim_begin__payload
{
    int order; // allocation order that triggered reclaim
};

struct sched__psi_memstall_enter__payload
{
    int type; /* 0 = some, 1 = full, etc. */
};

struct oom__mark_victim__payload
{
    // No additional fields required for this payload
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
        struct syscall__sys_enter_read__payload syscall__sys_enter_read__payload;
        struct syscall__sys_enter_write__payload syscall__sys_enter_write__payload;
        struct vmscan__mm_vmscan_direct_reclaim_begin__payload vmscan__mm_vmscan_direct_reclaim_begin__payload;
        struct sched__psi_memstall_enter__payload sched__psi_memstall_enter__payload;
        struct oom__mark_victim__payload oom__mark_victim__payload;
    };
} __attribute__((packed));

#endif /* BOOTSTRAP_H */