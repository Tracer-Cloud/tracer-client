#include "vmlinux.h"
#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>
#include "bootstrap.gen.h"

/* -------------------------------------------------------------------------- */
/*                TABLE OF CONTENTS (module subsections)                      */
/* -------------------------------------------------------------------------- */
/*  1. Maps - Data persisted across kernel invocations                        */
/*  2. Helper utilities - Common functions used throughout the code           */
/*  3. Event registration table - Tracepoint definitions and context types    */
/*  4. Payload fill functions - Event-specific data capture implementations   */
/*  5. Generic tracepoint generator - Macro-based event handler generation    */
/* -------------------------------------------------------------------------- */

/* -------------------------------------------------------------------------- */
/*                Maps. For data persisted across kernel invocations,         */
/*                      and 2-way communication with userspace.               */
/* -------------------------------------------------------------------------- */

// CPU-shared configuration map (PID blacklisting, debug mode, clock synchronization, etc)
// Values set from userspace
struct
{
  __uint(type, BPF_MAP_TYPE_ARRAY);
  __uint(max_entries, CONFIG_MAP_MAX_ENTRIES);
  __type(key, u32);
  __type(value, u64);
} config SEC(".maps");

// CPU-shared, sends event headers: contains common fields, payload index and flush signals
// Flushed on every event
struct
{
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, RINGBUF_MAX_ENTRIES);
} rb SEC(".maps");

// Sends payloads: variant-specific fields
struct
{
  __uint(type, BPF_MAP_TYPE_ARRAY);
  __uint(max_entries, PAYLOAD_BUFFER_N_ENTRIES_PER_CPU *MAX_CPUS); // max_entries has to be known at compile-time
  __type(key, u32);
  __type(value, u8[PAYLOAD_BUFFER_ENTRY_SIZE]);
} payload_buffer SEC(".maps")
    __attribute__((aligned(PAYLOAD_BUFFER_ENTRY_SIZE)));

// Per-CPU internal state (single map entry), persisted across kernel invocations
struct internal_state
{
  struct event_header_kernel *ringbuf_entry;
  u32 payload_entry_index;
  u32 payload_entry_start;
  u32 payload_entry_end;
  bool initialised;
};
struct
{
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, u32);
  __type(value, struct internal_state);
} internal_state_map SEC(".maps");
u32 internal_state_key = 0;

/* -------------------------------------------------------------------------- */
/*                            Helper utilities                                */
/* -------------------------------------------------------------------------- */

// Helper to read config values
static __always_inline u64 get_config(u32 key)
{
  u64 *value = bpf_map_lookup_elem(&config, &key);
  return value ? *value : 0;
}

// PIDs are not always sufficient to uniquely identify processes,
// because of PID reuse, so we combine with process start time
static __always_inline u64 make_upid(u32 pid, u64 start_ns)
{
  const u64 PID_MASK = 0x00FFFFFFULL;       /* 24 ones */
  const u64 TIME_MASK = 0x000FFFFFFFFFFULL; /* 40 ones */
  return ((u64)(pid & PID_MASK) << 40) | (start_ns & TIME_MASK);
}

// Skip threads and blacklisted PIDs
static __always_inline bool should_capture_event(void)
{
  u64 id = bpf_get_current_pid_tgid();
  u32 pid = id >> 32;
  u32 tid = (u32)id;

  // Ignore threads, report only the root process
  // TODO: handle multi-threaded processes
  if (pid != tid)
    return false;

  // Skip if PID is blacklisted
  // Check all blacklist entries (0-31)
  for (int i = 0; i < MAX_BLACKLIST_ENTRIES; i++)
  {
    u64 blacklisted_pid = get_config(CONFIG_PID_BLACKLIST_0 + i);
    // Early exit if we encounter a zero entry (end-of-list)
    if (unlikely(blacklisted_pid == 0))
      break;

    if (pid == (u32)blacklisted_pid)
    {
      return false;
    }
  }

  return true;
}

// Always skip read content
static __always_inline bool should_capture_read_content(void)
{
  return false;
}

// Capture stdout & stderr content
static __always_inline bool should_capture_write_content(struct payload_kernel_syscalls_sys_enter_write *p)
{
  // File descriptor 1 is stdout, 2 is stderr
  return p->fd == 1 || p->fd == 2;
}

// Creates a new event in the ringbuf and fills header, must be called before buf_malloc
static __always_inline struct event_header_kernel *create_event(enum event_type type)
{
  struct internal_state *state = bpf_map_lookup_elem(&internal_state_map, &internal_state_key);
  if (!state)
    return NULL;
  if (!state->initialised)
  {
    state->payload_entry_index = bpf_get_smp_processor_id() * PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
    state->payload_entry_start = state->payload_entry_index;
    state->payload_entry_end = state->payload_entry_index + PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
    state->initialised = true;
  }

  // Get task info
  u64 id = bpf_get_current_pid_tgid();
  u32 pid = id >> 32;

  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct task_struct *parent = BPF_CORE_READ(task, parent);
  u32 ppid = BPF_CORE_READ(parent, tgid);

  u64 start_ns = BPF_CORE_READ(task, start_time);
  u64 pstart_ns = BPF_CORE_READ(parent, start_time);
  u64 upid = make_upid(pid, start_ns);
  u64 uppid = make_upid(ppid, pstart_ns);

  // Reserve space in ringbuf for header
  struct event_header_kernel *current_header = bpf_ringbuf_reserve(&rb, sizeof(struct event_header_kernel), 0);
  if (!current_header)
  {
    state->ringbuf_entry = NULL;
    return NULL;
  }
  state->ringbuf_entry = current_header;

  // Fill header
  current_header->event_type = type;
  current_header->timestamp_ns = bpf_ktime_get_ns() + get_config(CONFIG_SYSTEM_BOOT_NS);
  current_header->pid = pid;
  current_header->ppid = ppid;
  current_header->upid = upid;
  current_header->uppid = uppid;
  BPF_CORE_READ_STR_INTO(&current_header->comm, task, comm); // first 16 bytes of comm only
  current_header->payload.start_index = state->payload_entry_index;
  current_header->payload.end_index = state->payload_entry_index;

  return current_header;
}

// Get a payload buffer entry and advance the index
static __always_inline void *get_payload_buf_entry()
{
  struct internal_state *state = bpf_map_lookup_elem(&internal_state_map, &internal_state_key);
  if (!state)
    return NULL;
  u32 key = state->payload_entry_index++;
  if (state->payload_entry_index >= state->payload_entry_end)
    state->payload_entry_index = state->payload_entry_start;
  return bpf_map_lookup_elem(&payload_buffer, &key);
}

// Read data from memory into payload buffer, with support for variable-length strings and arrays
// @src: Pointer to memory in userspace we should copy from
// @max_size: Maximum size of data to read (must be passed as compile-time constant, to satisfy verifier)
// @dyn_size: Size calculated at runtime; pass F_READ_NUL_TERMINATED to stop at first NUL (string termination)
static __always_inline int read_into_payload(void *src, u64 max_size, u64 dyn_size)
{
  bool is_nul_terminated = ((dyn_size & F_READ_NUL_TERMINATED) > 0);
  u32 max_entries = (max_size + PAYLOAD_BUFFER_ENTRY_SIZE - 1) / PAYLOAD_BUFFER_ENTRY_SIZE;

  if (is_nul_terminated)
  {
    for (int i = 0; i < max_entries; i++)
    {
      void *dst = get_payload_buf_entry();
      if (!dst)
        return -1;

      int ret = bpf_probe_read_user_str(dst, PAYLOAD_BUFFER_ENTRY_SIZE, src);
      if (ret < 0)
        return ret; // error
      else if (ret < PAYLOAD_BUFFER_ENTRY_SIZE)
        return i * PAYLOAD_BUFFER_ENTRY_SIZE + ret; // success

      src += PAYLOAD_BUFFER_ENTRY_SIZE; // advance pointer
    }
    return max_size;
  }
  else
  {
    dyn_size = dyn_size & 0xFFFFFFFF;                     // Mask lower 32 bits
    dyn_size = dyn_size > max_size ? max_size : dyn_size; // Clamp value
    u32 dyn_entries = (dyn_size + PAYLOAD_BUFFER_ENTRY_SIZE - 1) / PAYLOAD_BUFFER_ENTRY_SIZE;
    for (int i = 0; i < max_entries && i < dyn_entries; i++)
    {
      void *dst = get_payload_buf_entry();
      if (!dst)
        return -1;

      int ret = bpf_probe_read_user(dst, PAYLOAD_BUFFER_ENTRY_SIZE, src);
      if (ret < 0)
        return ret; // error

      src += PAYLOAD_BUFFER_ENTRY_SIZE; // advance pointer
    }
    return dyn_size;
  }
}

// Wrapper around read_into_payload that updates attribute descriptor with location
static __always_inline int read_into_attr(void *src, u64 max_size, u64 dyn_size, u64 *attr)
{
  if (max_size > PAYLOAD_BUFFER_N_ENTRIES_PER_CPU * PAYLOAD_BUFFER_ENTRY_SIZE)
    return -1;
  struct internal_state *state = bpf_map_lookup_elem(&internal_state_map, &internal_state_key);
  if (!state)
    return -1;
  u32 byte_index = state->payload_entry_index * PAYLOAD_BUFFER_ENTRY_SIZE;
  int ret = read_into_payload(src, max_size, dyn_size);
  if (ret <= 0)
  {
    return ret; // Error
  }
  u32 byte_length = (u32)ret;
  *attr = ((u64)byte_index << 32) | byte_length;
  return ret;
}

// Submit an event to ringbuf, called after all data has been captured.
static __always_inline void submit_event(struct event_header_kernel *current_header)
{
  if (!current_header)
    return;

  struct internal_state *state = bpf_map_lookup_elem(&internal_state_map, &internal_state_key);
  if (state)
  {
    current_header->payload.end_index = state->payload_entry_index;
  }

  // Submit event header via ringbuf
  bpf_ringbuf_submit(current_header, 0);
}

/* --------------------------------------------------------------------------------- */
/*                             Event registration table                              */
/*      Entries include category name, tracepoint name, and context struct type      */
/* --------------------------------------------------------------------------------- */

// Forward declarations to fix compiler warnings
struct trace_event_raw_psi_memstall;
struct trace_event_raw_vmscan_direct_reclaim_begin;
struct trace_event_raw_mark_victim;

// Event list
#define TRACEPOINT_LIST(X)                                                               \
  X(sched, sched_process_exec, trace_event_raw_sched_process_exec)                       \
  X(sched, sched_process_exit, trace_event_raw_sched_process_template)                   \
  /* TODO: cannot attach psi_memstall_enter */                                           \
  /* X(sched, psi_memstall_enter, trace_event_raw_psi_memstall) */                       \
                                                                                         \
  X(syscalls, sys_enter_openat, trace_event_raw_sys_enter)                               \
  X(syscalls, sys_exit_openat, trace_event_raw_sys_exit)                                 \
  X(syscalls, sys_enter_read, trace_event_raw_sys_enter)                                 \
  X(syscalls, sys_enter_write, trace_event_raw_sys_enter)                                \
                                                                                         \
  X(vmscan, mm_vmscan_direct_reclaim_begin, trace_event_raw_vmscan_direct_reclaim_begin) \
  X(oom, mark_victim, trace_event_raw_mark_victim)

/* -------------------------------------------------------------------------- */
/*                           Payload fill functions                           */
/* -------------------------------------------------------------------------- */

// Process execution (successful)
static __always_inline void
payload_fill_sched_sched_process_exec(struct trace_event_raw_sched_process_exec *ctx)
{
  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  struct mm_struct *mm = BPF_CORE_READ(task, mm); // memory map
  if (!mm)
    return;

  struct payload_kernel_sched_sched_process_exec *p = get_payload_buf_entry();
  if (!p)
    return;

  // argv stored in user memory as nul-separated strings
  u64 argv_start = BPF_CORE_READ(mm, arg_start);
  u64 argv_end = BPF_CORE_READ(mm, arg_end);
  u64 argv_size = argv_end - argv_start;
  read_into_attr((void *)argv_start, ARGV_MAX_SIZE, argv_size, &p->argv);
}

// Process termination (successful)
static __always_inline void
payload_fill_sched_sched_process_exit(struct trace_event_raw_sched_process_template *ctx)
{
  struct payload_kernel_sched_sched_process_exit *p = get_payload_buf_entry();
  if (!p)
    return;

  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  p->exit_code = BPF_CORE_READ(task, exit_code);
}

// Memory pressure, stall begins
static __always_inline void
payload_fill_sched_psi_memstall_enter(struct trace_event_raw_psi_memstall *ctx)
{
  struct payload_kernel_sched_psi_memstall_enter *p = get_payload_buf_entry();
  if (!p)
    return;

  // TODO: cannot read ctx->type, seems to be undefined on trace_event_raw_psi_memstall
  // p->type = BPF_CORE_READ(ctx, type);
}

// File open, syscall entry
static __always_inline void
payload_fill_syscalls_sys_enter_openat(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_openat *p = get_payload_buf_entry();
  if (!p)
    return;

  p->dfd = BPF_CORE_READ(ctx, args[0]);
  p->flags = BPF_CORE_READ(ctx, args[2]);
  p->mode = BPF_CORE_READ(ctx, args[3]);

  void *content_ptr = (void *)BPF_CORE_READ(ctx, args[1]);
  read_into_attr(content_ptr, FILENAME_MAX_SIZE, F_READ_NUL_TERMINATED, &p->filename);
}

// File open, syscall return
static __always_inline void
payload_fill_syscalls_sys_exit_openat(struct trace_event_raw_sys_exit *ctx)
{
  struct payload_kernel_syscalls_sys_exit_openat *p = get_payload_buf_entry();
  if (!p)
    return;

  p->fd = BPF_CORE_READ(ctx, ret);
}

// Files and pipes, read syscall entry
static __always_inline void
payload_fill_syscalls_sys_enter_read(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_read *p = get_payload_buf_entry();
  if (!p)
    return;

  p->fd = BPF_CORE_READ(ctx, args[0]);
  p->count = BPF_CORE_READ(ctx, args[2]);
}

// File read completed - empty payload
static __always_inline void
payload_fill_syscalls_sys_exit_read(struct trace_event_raw_sys_exit *ctx)
{
  // Empty payload, no need to reserve buffer space
}

// Files and pipes, write syscall entry
static __always_inline void
payload_fill_syscalls_sys_enter_write(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_write *p = get_payload_buf_entry();
  if (!p)
    return;

  p->fd = BPF_CORE_READ(ctx, args[0]);
  p->content = 0; // No descriptor
  p->count = BPF_CORE_READ(ctx, args[2]);

  // Capture content only for stdout/stderr
  if (should_capture_write_content(p) && p->count > 0)
  {
    u64 *content_ptr = (u64 *)BPF_CORE_READ(ctx, args[1]);
    read_into_attr(content_ptr, WRITE_CONTENT_MAX_SIZE, p->count, &p->content);
  }
}

// File write completed - empty payload
static __always_inline void
payload_fill_syscalls_sys_exit_write(struct trace_event_raw_sys_exit *ctx)
{
  // Empty payload, no need to reserve buffer space
}

// Memory pressure, reclaim begins
static __always_inline void
payload_fill_vmscan_mm_vmscan_direct_reclaim_begin(struct trace_event_raw_vmscan_direct_reclaim_begin *ctx)
{
  struct payload_kernel_vmscan_mm_vmscan_direct_reclaim_begin *p =
      get_payload_buf_entry();
  if (!p)
    return;

  // TODO: cannot read ctx->order, seems to be undefined on trace_event_raw_vmscan_direct_reclaim_begin
  // p->order = BPF_CORE_READ(ctx, order);
}

// Memory pressure, OOM killer selects process
static __always_inline void
payload_fill_oom_mark_victim(struct trace_event_raw_mark_victim *ctx)
{
  // No fields to fill for OOM mark victim
}

/* -------------------------------------------------------------------------- */
/*                       Generic tracepoint generator                         */
/* -------------------------------------------------------------------------- */

#define HANDLER_DECL(cat, tracepoint, ctx_t)                                                    \
  SEC("tracepoint/" #cat "/" #tracepoint)                                                       \
  int handle_##cat##_##tracepoint(struct ctx_t *ctx)                                            \
  {                                                                                             \
    if (!should_capture_event())                                                                \
      return 0;                                                                                 \
                                                                                                \
    /* Create event - reserves space in ringbuf and fills header */                             \
    struct event_header_kernel *current_header = create_event(event_type_##cat##_##tracepoint); \
    if (!current_header)                                                                        \
      return 0;                                                                                 \
                                                                                                \
    /* Call variant-specific function to write payload data */                                  \
    payload_fill_##cat##_##tracepoint(ctx);                                                     \
                                                                                                \
    /* Submit event to ringbuf */                                                               \
    submit_event(current_header);                                                               \
                                                                                                \
    return 0;                                                                                   \
  }

/* Instantiate one handler per TRACEPOINT_LIST entry */
TRACEPOINT_LIST(HANDLER_DECL)
#undef HANDLER_DECL

// License (applies to this file only), required to invoke GPL-restricted BPF functions
char LICENSE[] SEC("license") = "GPL";