// templ_start:file_description
/* ========================================================================== */
/*                            TEMPLATE FILE                                   */
/* ========================================================================== */
/*                                                                            */
/*  This template is used to generate bootstrap.gen.h                         */
/*  REGENERATE AFTER EDITING - changes will have no effect                    */
/*                                                                            */
/*  Generator: ebpf/typegen/typegen.rs                                        */
/*  Template:  ebpf/c/bootstrap.templ.h                                       */
/*  Config:    ebpf/typegen/events.toml                                       */
/*                                                                            */
/*  To regenerate: `cd tracer-client/src/ebpf/c && make` (fast)               */
/*  Alternative:   `cd tracer-client && cargo build` (slower)                 */
/*                                                                            */
/* ========================================================================== */
// templ_end:file_description

#ifndef BOOTSTRAP_H
#define BOOTSTRAP_H

typedef unsigned long long u64;
typedef unsigned int u32;
typedef unsigned short u16;
typedef unsigned char u8;

// Simple strcpy implementation for BPF (no string.h needed)
static inline char *bpf_strcpy(char *dest, const char *src)
{
  char *d = dest;
  while ((*d++ = *src++))
    ;
  return dest;
}

// Map configuration constants
#define CONFIG_MAP_MAX_ENTRIES 64                    // 64 * 8 bytes for blacklist, config settings, etc
#define RINGBUF_MAX_ENTRIES (256 * 1024)             // 256KB * sizeof(event_header_kernel)
#define PAYLOAD_BUFFER_ENTRY_SIZE 64                 // 64 bytes (smallest unit allowing fast copies)
#define PAYLOAD_BUFFER_N_ENTRIES_PER_CPU (16 * 1024) // 16 * 1024 * sizeof(entry) = 1MB
#define PAYLOAD_FLUSH_TIMEOUT_NS 750000000ULL        // 750 milliseconds (latency upper bound)
#define MAX_CPUS 256                                 // Maximum CPUs supported for manual per-CPU isolation
#define F_READ_NUL_TERMINATED (1ULL << 32)           // Flag for read_into_payload()

// Memory and string size constants
#define TASK_COMM_LEN 16  // Non-essential value, possibly trimmed
#define ARGV_MAX_SIZE 384 // 256+128 bytes (uses 75% of available in-kernel memory)
#define FILENAME_MAX_SIZE 384
#define WRITE_CONTENT_MAX_SIZE 32768 // Maximum bytes to capture from stdout/stderr

// Map keys for configuration values
#define CONFIG_PID_BLACKLIST_0 0
// CONFIG_PID_BLACKLIST_0..31 implicitly defined as CONFIG_PID_BLACKLIST_0 + 0..MAX_BLACKLIST_ENTRIES
#define MAX_BLACKLIST_ENTRIES 32
#define CONFIG_DEBUG_ENABLED 32
#define CONFIG_SYSTEM_BOOT_NS 33 // Needed for timestamps

// The exact values for event IDs are chosen arbitrarily, but should stay consistent between Tracer versions
// templ_start:event_type
enum event_type
{
};
// templ_end:event_type

// Attributes common to every event
struct event_header_user
{
  u64 event_id;
  enum event_type event_type;
  u64 timestamp_ns;
  u32 pid;
  u32 ppid;
  u64 upid;
  u64 uppid;
  char comm[TASK_COMM_LEN];
  void *payload;
} __attribute__((packed));
struct event_header_kernel
{
  struct
  {
    u32 start_index;
    u32 end_index;
  } payload;
  enum event_type event_type;
  u64 timestamp_ns;
  u32 pid;
  u32 ppid;
  u64 upid;
  u64 uppid;
  char comm[TASK_COMM_LEN];
} __attribute__((packed));

struct flex_buf
{
  u32 byte_length;
  char *data;
} __attribute__((packed));

// templ_start:payload_structs
// templ_end:payload_structs

// Helper for collapsing kernel-provided allocation chains into a single node
struct dar_array
{
  u32 length;
  u64 *data; // pointers to root descriptors
};

// Get pointers to dynamic payload attributes (ie, strings and arrays of compile-time-unknown size)
// templ_start:payload_to_dynamic_allocation_roots
static inline void
payload_to_dynamic_allocation_roots(enum event_type t,
                                    void *src_ptr,
                                    void *dst_ptr,
                                    struct dar_array *src_result,
                                    struct dar_array *dst_result)
{
  *src_result = (struct dar_array){0, NULL};
  *dst_result = (struct dar_array){0, NULL};
  switch (t)
  {
  default:
    break;
  }
}
// templ_end:payload_to_dynamic_allocation_roots

// For the statically measurable part of payloads only
// templ_start:get_payload_size
static inline size_t get_payload_fixed_size(enum event_type t)
{
  switch (t)
  {
  default:
    return 0;
  }
}
// templ_end:get_payload_size

// -------
// Helpers for printing as JSON (in example.cpp)
// -------

// templ_start:event_type_to_string
static inline const char *event_type_to_string(enum event_type t)
{
  switch (t)
  {
  default:
    return "unknown";
  }
}
// templ_end:event_type_to_string

struct kv_entry
{
  char type[32]; // eg, "u32"
  char key[32];  // eg, "filename"
  void *value;
};

struct kv_array
{
  u32 length;
  struct kv_entry *data;
};

// templ_start:payload_to_kv_array
static inline struct kv_array payload_to_kv_array(enum event_type t, void *ptr)
{
  struct kv_array result = {0, NULL};
  switch (t)
  {
  default:
    break;
  }
  return result;
}
// templ_end:payload_to_kv_array

#endif /* BOOTSTRAP_H */
