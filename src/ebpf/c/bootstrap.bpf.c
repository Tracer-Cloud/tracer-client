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

// Sends payloads: variant-specific fields. Each entry is a large page, writable via buf_malloc
// CPU X owns keys [X * PAYLOAD_BUFFER_N_PAGES, (X+1) * PAYLOAD_BUFFER_N_PAGES)
// Pages flushed when full or upon timeout
struct
{
  __uint(type, BPF_MAP_TYPE_ARRAY);
  __uint(max_entries, PAYLOAD_BUFFER_N_PAGES *MAX_CPUS); // max_entries has to be known at compile-time
  __type(key, u32);
  __type(value, u8[PAGE_SIZE]); // 4KB = standard page size (hardware)
} payload_buffer SEC(".maps")
    __attribute__((aligned(PAGE_SIZE))); // Reduces TLB and store/load buffer switches

// Per-CPU internal state (single map entry), persisted across kernel invocations
// Access with `struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);`
struct buffer_state
{
  u32 current_page;            // Current page index being written to
  u32 current_offset;          // Current offset within the page
  u64 page_start_timestamp_ns; // Timestamp when current page was started
};
struct
{
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, u32);
  __type(value, struct buffer_state);
} buffer_state_map SEC(".maps");
u32 buffer_state_key = 0;

// Per-CPU internal state NOT persisted across kernel invocations
struct
{
  u64 *root_descriptor;             // Root descriptor for the allocation chain
  u32 chain_descriptor_page_index;  // Page index of current descriptor
  u32 chain_descriptor_page_offset; // Page offset of current descriptor
  u16 depth;                        // Current depth in the allocation chain
  bool is_final;                    // Chain is finalised
  u16 order;                        // Order of the allocation for attribute ordering
  u32 size;                         // Size of the current allocation
} prev_malloc_dyn = {
    .root_descriptor = NULL,
    .chain_descriptor_page_index = 0,
    .chain_descriptor_page_offset = 0,
    .depth = 0,
    .is_final = true,
    .order = 0,
    .size = 0};

/* -------------------------------------------------------------------------- */
/*                            Helper utilities                                */
/* -------------------------------------------------------------------------- */

// Helper to read config values
static __always_inline u64 get_config(u32 key)
{
  u64 *value = bpf_map_lookup_elem(&config, &key);
  return value ? *value : 0;
}

// Helper to calculate CPU-specific key for the payload_buffer map
static __always_inline u32 cpu_page_key(u32 page_index)
{
  u32 cpu = bpf_get_smp_processor_id();
  return cpu * PAYLOAD_BUFFER_N_PAGES + (page_index % PAYLOAD_BUFFER_N_PAGES);
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
  // Reset state
  prev_malloc_dyn.root_descriptor = NULL;
  prev_malloc_dyn.chain_descriptor_page_index = 0;
  prev_malloc_dyn.chain_descriptor_page_offset = 0;
  prev_malloc_dyn.depth = 0;
  prev_malloc_dyn.is_final = true;
  prev_malloc_dyn.order = 0;
  prev_malloc_dyn.size = 0;

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
    return NULL;

  // Fill header
  current_header->event_type = type;
  current_header->timestamp_ns = bpf_ktime_get_ns() + get_config(CONFIG_SYSTEM_BOOT_NS);
  current_header->pid = pid;
  current_header->ppid = ppid;
  current_header->upid = upid;
  current_header->uppid = uppid;

  // Get process name (first 16 bytes, possibly trimmed)
  BPF_CORE_READ_STR_INTO(&current_header->comm, task, comm);

  // Fill payload index
  current_header->payload.cpu = bpf_get_smp_processor_id();
  struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
  if (state)
  {
    current_header->payload.page_index = state->current_page;
    current_header->payload.byte_offset = state->current_offset;
  }

  return current_header;
}

static __always_inline u32 current_page_available_space()
{
  struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
  if (!state)
    return 0;
  return PAGE_SIZE - state->current_offset;
}

// Helper to reserve buffer space for data.
// Use directly for fixed-size payloads, and via buf_malloc_dyn for arrays and strings of compile-time unknown length.
static __always_inline void *buf_malloc(u64 size)
{
  // Validate max allocation = 4KB
  if (size > PAGE_SIZE)
  {
    bpf_printk("Max allocation is %u bytes; attempted to allocate %u bytes", PAGE_SIZE, size);
    return NULL;
  }

  struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
  if (!state)
    return NULL;

  u64 current_time_ns = bpf_ktime_get_ns();

  // Initialize timestamp for the very first allocation
  if (unlikely(state->page_start_timestamp_ns == 0))
  {
    state->page_start_timestamp_ns = current_time_ns;
  }

  // Maybe start new page (due to size or timeout)
  if (state->current_offset + size >= PAGE_SIZE ||
      (current_time_ns - state->page_start_timestamp_ns) > PAYLOAD_FLUSH_TIMEOUT_NS)
  {
    if (state->current_offset + 8 <= PAGE_SIZE)
    {
      // Zero out 8 bytes at current_offset to prevent false positives when
      // seeking dynamic allocation descriptors in userspace
      u32 key = cpu_page_key(state->current_page);
      u8(*page)[PAGE_SIZE] = bpf_map_lookup_elem(&payload_buffer, &key);
      if (page && state->current_offset <= PAGE_SIZE - 8)
      {
        u64 *clear_ptr = (u64 *)&(*page)[state->current_offset];
        *clear_ptr = 0;
      }
    }

    state->current_page = (state->current_page + 1) % PAYLOAD_BUFFER_N_PAGES;
    state->current_offset = 0;
    state->page_start_timestamp_ns = current_time_ns;
  }

  // Get pointer to reserved space
  u32 key = cpu_page_key(state->current_page);
  u8(*page)[PAGE_SIZE] = bpf_map_lookup_elem(&payload_buffer, &key);
  if (!page)
    return NULL;

  // Help verifier understand the offset is bounded with redundant check
  u32 offset = state->current_offset;
  if (offset > PAGE_SIZE - size)
    return NULL;

  // Get location where data can be written
  void *ptr = &(*page)[offset];

  // Update offset for next allocation
  state->current_offset = offset + size;

  return ptr;
}

// Wrapper around buf_malloc that adds support for arrays and strings of variable size,
// including support for splitting data across multiple allocations.
static __always_inline void *buf_malloc_dyn(u32 size, bool is_final, u64 *descriptor)
{
  bool is_first = (prev_malloc_dyn.is_final == true);
  void *ptr;

  if (!is_first && prev_malloc_dyn.root_descriptor != descriptor)
  {
    bpf_printk("Previous dynamic memory allocation must be finalised before starting a new allocation");
    return NULL;
  }

  if (is_first)
  {
    // Root allocation - reserve space for data only
    ptr = buf_malloc(size);
    if (!ptr)
      return NULL;

    // Used when there are multiple dynamic attributes in one event
    // and we need to determine memory layout
    u16 order = ++prev_malloc_dyn.order;

    // Write root descriptor: [is_final:1][order:16][size:47]
    *descriptor = ((u64)is_final << 63) | ((u64)order << 47) | ((u64)size & 0x7FFFFFFFFFFFULL);
    prev_malloc_dyn.root_descriptor = descriptor;
    prev_malloc_dyn.depth = 1;
  }
  else
  {
    // Get current state
    struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
    if (!state)
      return NULL;

    prev_malloc_dyn.chain_descriptor_page_index = state->current_page;
    prev_malloc_dyn.chain_descriptor_page_offset = state->current_offset;
    prev_malloc_dyn.depth++;

    // Allocate space for chain + data
    ptr = buf_malloc(size + 8);
    if (!ptr)
      return NULL;

    // Write chain pointer at the beginning of this allocation
    u64 *chain_ptr = (u64 *)ptr;
    *chain_ptr = ((u64)is_final << 63) | ((u64)prev_malloc_dyn.order << 47) | ((u64)size & 0x7FFFFFFFFFFFULL);

    // Return pointer to data (after chain pointer)
    ptr = (char *)ptr + 8;
  }

  prev_malloc_dyn.is_final = is_final;
  prev_malloc_dyn.size = size;

  return ptr;
}

// Shrink the last dynamic allocation to the specified size
// This updates the descriptor and reclaims unused buffer space
static __always_inline void buf_malloc_dyn_shrink_last(u32 new_size, u64 *root_descriptor)
{
  if (!prev_malloc_dyn.root_descriptor)
    return;

  struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
  if (!state)
    return;

  u32 current_size = (prev_malloc_dyn.size);

  // Only shrink if new size is smaller than current size
  if (new_size >= current_size)
    return;
  u32 size_diff = current_size - new_size;

  // Update buffer state to reclaim space
  if (state->current_offset >= size_diff)
    state->current_offset -= size_diff;

  u64 *descriptor_ptr;

  if (prev_malloc_dyn.depth == 1)
  {
    descriptor_ptr = root_descriptor;
  }
  else
  {
    // Chain descriptor pointers have to be reconstructed using page index and offset
    // so BPF verifier can track bounds and confirm safety
    u32 key = cpu_page_key(prev_malloc_dyn.chain_descriptor_page_index);
    u8(*page)[PAGE_SIZE] = bpf_map_lookup_elem(&payload_buffer, &key);
    if (!page || prev_malloc_dyn.chain_descriptor_page_offset > PAGE_SIZE - 8)
      return;
    u64 *descriptor_ptr = (u64 *)&(*page)[prev_malloc_dyn.chain_descriptor_page_offset];
  }

  // TODO: descriptor_ptr always seems to point to root descriptor(?)

  // Update descriptor
  *descriptor_ptr = ((u64)prev_malloc_dyn.is_final << 63) | ((u64)prev_malloc_dyn.order << 47) | ((u64)new_size & 0x7FFFFFFFFFFFULL);

  // Update tracking size
  prev_malloc_dyn.size = new_size;
}

// Submit an event to ringbuf, called after all data has been captured.
static __always_inline void submit_event(struct event_header_kernel *current_header)
{
  if (!current_header)
    return;

  struct buffer_state *state = bpf_map_lookup_elem(&buffer_state_map, &buffer_state_key);
  if (state)
  {
    // Tell userspace how many pages of payload data the kernel wants to flush
    u16 first_page = current_header->payload.page_index;
    u16 current_page = state->current_page;
    if (current_page < first_page)
    {
      current_page += PAYLOAD_BUFFER_N_PAGES; // Started new cycle through page buffer
    }
    current_header->payload.flush_signal = current_page - first_page;
  }
  else
  {
    current_header->payload.flush_signal = 0;
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
  struct mm_struct *mm; // memory map
  u32 i, offset = 0;

  mm = BPF_CORE_READ(task, mm);
  if (!mm)
    return;

  u64 argv_start = BPF_CORE_READ(mm, arg_start);
  u64 argv_end = BPF_CORE_READ(mm, arg_end);
  u64 argv_size = argv_end - argv_start;
  argv_size = 0 <= argv_size && argv_size <= ARGV_MAX_SIZE ? argv_size : ARGV_MAX_SIZE;

  struct payload_kernel_sched_sched_process_exec *p = buf_malloc(sizeof(struct payload_kernel_sched_sched_process_exec));
  if (!p)
    return;

  // native format (null-separated strings) for sched_process_exec.argv matches our str[][] format,
  // so we can copy the entire array from memory directly and without modification
  char *payload_argv = buf_malloc_dyn(ARGV_MAX_SIZE, true, &p->argv);
  if (payload_argv)
  {
    // TODO: read args one-at-a-time: current solution only works if every page in
    // the [arg_start … arg_end) range is mapped and already resident in the
    // new process's address-space. We sometimes get corrupted data with current solution.
    bpf_probe_read_user(payload_argv, argv_size, (void *)argv_start);
    buf_malloc_dyn_shrink_last(argv_size, &p->argv);
  }
}

// Process termination (successful)
static __always_inline void
payload_fill_sched_sched_process_exit(struct trace_event_raw_sched_process_template *ctx)
{
  struct payload_kernel_sched_sched_process_exit *p = buf_malloc(sizeof(struct payload_kernel_sched_sched_process_exit));
  if (!p)
    return;

  struct task_struct *task = (struct task_struct *)bpf_get_current_task();
  p->exit_code = BPF_CORE_READ(task, exit_code);
}

// Memory pressure, stall begins
static __always_inline void
payload_fill_sched_psi_memstall_enter(struct trace_event_raw_psi_memstall *ctx)
{
  struct payload_kernel_sched_psi_memstall_enter *p = buf_malloc(sizeof(struct payload_kernel_sched_psi_memstall_enter));
  if (!p)
    return;

  // TODO: cannot read ctx->type, seems to be undefined on trace_event_raw_psi_memstall
  // p->type = BPF_CORE_READ(ctx, type);
}

// File open, syscall entry
static __always_inline void
payload_fill_syscalls_sys_enter_openat(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_openat *p = buf_malloc(sizeof(struct payload_kernel_syscalls_sys_enter_openat));
  if (!p)
    return;

  p->dfd = BPF_CORE_READ(ctx, args[0]);
  p->flags = BPF_CORE_READ(ctx, args[2]);
  p->mode = BPF_CORE_READ(ctx, args[3]);

  // Capture filename using buf_malloc_dyn_first
  // First, read filename into temporary buffer to get the actual length
  void *filename_payload = buf_malloc_dyn(FILENAME_MAX_SIZE, true, &p->filename);
  if (filename_payload)
  {
    int filename_len = bpf_probe_read_user_str(filename_payload, FILENAME_MAX_SIZE, (void *)BPF_CORE_READ(ctx, args[1]));
    buf_malloc_dyn_shrink_last(filename_len, &p->filename);
  }
}

// File open, syscall return
static __always_inline void
payload_fill_syscalls_sys_exit_openat(struct trace_event_raw_sys_exit *ctx)
{
  struct payload_kernel_syscalls_sys_exit_openat *p = buf_malloc(sizeof(struct payload_kernel_syscalls_sys_exit_openat));
  if (!p)
    return;

  p->fd = BPF_CORE_READ(ctx, ret);
}

// Files and pipes, read syscall entry
static __always_inline void
payload_fill_syscalls_sys_enter_read(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_read *p = buf_malloc(sizeof(struct payload_kernel_syscalls_sys_enter_read));
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
  struct payload_kernel_syscalls_sys_enter_write *p = buf_malloc(sizeof(struct payload_kernel_syscalls_sys_enter_write));
  if (!p)
    return;

  p->fd = BPF_CORE_READ(ctx, args[0]);
  p->content = 0; // No descriptor
  p->count = BPF_CORE_READ(ctx, args[2]);

  // Capture content only for stdout/stderr
  if (should_capture_write_content(p) && p->count > 0)
  {
    u64 content_ptr = BPF_CORE_READ(ctx, args[1]);
    u32 content_offset = 0;
    u32 remaining_size = (u32)p->count;

    // Capture content in chunks using chained allocations
    // Use bounded loop to satisfy BPF verifier constraints
    for (int i = 0; i < WRITE_CONTENT_N_ALLOCATIONS_MAX; i++)
    {
      u32 chunk_size = (remaining_size < WRITE_CONTENT_ALLOCATION_MAX_SIZE) ? remaining_size : WRITE_CONTENT_ALLOCATION_MAX_SIZE;
      bool is_final = (remaining_size <= WRITE_CONTENT_ALLOCATION_MAX_SIZE) || i == (WRITE_CONTENT_N_ALLOCATIONS_MAX - 1);

      void *content_chunk = buf_malloc_dyn(WRITE_CONTENT_ALLOCATION_MAX_SIZE, is_final, &p->content);
      if (!content_chunk)
        break;

      int err = bpf_probe_read_user(content_chunk, WRITE_CONTENT_ALLOCATION_MAX_SIZE, (void *)(content_ptr + content_offset));
      if (err != 0)
        break;

      content_offset += chunk_size;
      remaining_size -= chunk_size;

      if (is_final)
      {
        // buf_malloc_dyn_shrink_last(chunk_size, &p->content);
        break;
      }
    }
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
      buf_malloc(sizeof(struct payload_kernel_vmscan_mm_vmscan_direct_reclaim_begin));
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