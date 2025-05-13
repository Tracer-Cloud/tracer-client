// #include <linux/bpf.h>
// #include <linux/ptrace.h>
// #include <bpf/bpf_helpers.h>

// struct event {
//     u32 pid;
//     char comm[16];
//     char filename[256];
// };

// struct {
//     __uint(type, BPF_MAP_TYPE_PERF_EVENT_ARRAY);
//     __uint(key_size, sizeof(int));
//     __uint(value_size, sizeof(u32));
//     __uint(max_entries, 1024);
// } events SEC(".maps");

// SEC("tracepoint/syscalls/sys_enter_execve")
// int trace_execve(struct trace_event_raw_sys_enter *ctx)
// {
//     struct event event = {};

//     event.pid = bpf_get_current_pid_tgid() >> 32;
//     bpf_get_current_comm(&event.comm, sizeof(event.comm));

//     const char *filename = (const char *)ctx->args[0];
//     bpf_probe_read_user_str(event.filename, sizeof(event.filename), filename);

//     bpf_perf_event_output(ctx, &events, BPF_F_CURRENT_CPU, &event, sizeof(event));

//     return 0;
// }

// char LICENSE[] SEC("license") = "GPL";
