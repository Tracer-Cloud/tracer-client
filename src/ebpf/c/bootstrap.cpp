#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>
#include <iostream>
#include <cstring>
#include <dirent.h>
#include <fstream>
#include <vector>
#include <regex>
#include <algorithm>
#include <cassert>
#include <random>
#include <unordered_map>
#include <unordered_set>

#include <bpf/libbpf.h>
#include <bpf/bpf.h>

#include "bootstrap.gen.h"
#include "bootstrap.skel.h"
#include "bootstrap-api.h"
#include "bootstrap-filter.hpp"

#ifndef likely
#define likely(x) __builtin_expect(!!(x), 1)
#endif
#ifndef unlikely
#define unlikely(x) __builtin_expect(!!(x), 0)
#endif

// Define missing constants
#define PAGE_SIZE 4096
#define PAYLOAD_FLUSH_MAX_PAGES 256 // Increased from 128 to handle full 1MB per-CPU buffer (16K * 64 bytes = 1MB)

/* -------------------------------------------------------------------------- */
/*                                 Helpers                                    */
/* -------------------------------------------------------------------------- */

static struct env
{
  bool verbose;
  bool debug_bpf;
} env = {
    .verbose = false,
    .debug_bpf = false,
};

// Event ID generation
static std::random_device rd;
static std::mt19937_64 rng(rd());
static u64 event_id_base = rng();
static u64 event_id_counter = 0;
static u64 generate_event_id()
{
  return event_id_base + (++event_id_counter);
}

// Minimal context needed to associate event headers and payloads
struct pending_payload_info
{
  u64 event_id;
  enum event_type event_type;
  u16 page_index;
  u16 page_offset;
};

// Extended library context to handle per-CPU arrays and simplified 2-layer buffering
struct lib_ctx
{
  // Context structures for the new API
  header_ctx *header_ctx_ptr;
  payload_ctx *payload_ctx_ptr;
  event_callback_t event_cb;

  // Other context
  struct bootstrap_bpf *skel; // BPF skeleton
  struct ring_buffer *rb;     // Ring buffer
  int payload_buffer_fd;      // Shared array map fd (with manual CPU isolation)

  // Reusable payload buffer for direct processing
  u8 current_payload_flush[PAYLOAD_FLUSH_MAX_PAGES * PAGE_SIZE];
};

/* Find when the host system booted */
static u64 get_system_boot_ns(void)
{
  struct timespec realtime, monotonic;
  u64 realtime_ns, monotonic_ns;

  clock_gettime(CLOCK_REALTIME, &realtime);
  clock_gettime(CLOCK_MONOTONIC, &monotonic);

  realtime_ns = realtime.tv_sec * 1000000000ULL + realtime.tv_nsec;
  monotonic_ns = monotonic.tv_sec * 1000000000ULL + monotonic.tv_nsec;

  return realtime_ns - monotonic_ns;
}

static int libbpf_print_cb(enum libbpf_print_level lvl,
                           const char *fmt,
                           va_list args)
{
  if (lvl == LIBBPF_DEBUG && !env.verbose)
    return 0;
  return vfprintf(stderr, fmt, args);
}

static volatile bool exiting;

static void sig_handler(int sig) { exiting = true; }

/* -------------------------------------------------------------------------- */
/*                            Event processing                                */
/* -------------------------------------------------------------------------- */

// Ring‑buffer callback for processing headers, flush initiated by kernel
static int handle_header_flush(void *ctx, void *data, size_t _)
{
  if (!ctx || !data)
  {
    std::cerr << "Error: Invalid context or data in handle_header_flush" << std::endl;
    return -1;
  }

  struct lib_ctx *lc = static_cast<struct lib_ctx *>(ctx);

  if (!lc->header_ctx_ptr || !lc->header_ctx_ptr->data || !lc->payload_ctx_ptr || !lc->event_cb)
  {
    std::cerr << "Error: Invalid library context in handle_header_flush" << std::endl;
    return -1;
  }

  struct event_header_kernel *kern_header = static_cast<struct event_header_kernel *>(data);

  if (bootstrap_filter__should_skip(kern_header))
  {
    return 0;
  }

  // Generate event ID
  u64 event_id = generate_event_id();

  // Copy header data to header_ctx
  *lc->header_ctx_ptr->data = *((struct event_header_user *)kern_header);
  lc->header_ctx_ptr->data->event_id = event_id;

  // Calculate positions within this CPU's buffer
  int raw_start_idx = kern_header->payload.start_index;
  int raw_end_idx = kern_header->payload.end_index;

  int cpu_base = raw_start_idx - raw_start_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
  int start_in_cpu = raw_start_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
  int end_in_cpu = raw_end_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;

  int payload_size = end_in_cpu - start_in_cpu;
  if (start_in_cpu > end_in_cpu)
    payload_size += PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;

  // Copy payload entries to temporary buffer
  for (u32 i = 0; i < payload_size; i++)
  {
    // Bounds check to prevent buffer overflow
    if (i * PAYLOAD_BUFFER_ENTRY_SIZE >= sizeof(lc->current_payload_flush))
    {
      std::cerr << "Error: Payload copy would overflow buffer at index " << i << std::endl;
      break;
    }

    u32 map_index = cpu_base + (start_in_cpu + i) % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
    void *dst = &lc->current_payload_flush[i * PAYLOAD_BUFFER_ENTRY_SIZE];

    int lookup_result = bpf_map_lookup_elem(lc->payload_buffer_fd, &map_index, dst);
    if (lookup_result != 0)
    {
      std::cerr << "Error: bpf_map_lookup_elem failed for index " << map_index << std::endl;
    }
  }

  if (payload_size <= 0)
  {
    // No payload - just notify consumer of header data
    lc->payload_ctx_ptr->event_id = event_id;
    lc->payload_ctx_ptr->event_type = kern_header->event_type;
    lc->payload_ctx_ptr->data = nullptr;

    lc->event_cb(lc->header_ctx_ptr, lc->payload_ctx_ptr);
    return 0;
  }

  // Set up payload context for single payload
  lc->payload_ctx_ptr->event_id = event_id;
  lc->payload_ctx_ptr->event_type = kern_header->event_type;

  // Ensure enough space is available for the payload
  size_t payload_fixed_size = get_payload_fixed_size(kern_header->event_type);

  if (!lc->payload_ctx_ptr->data)
  {
    std::cerr << "Error: payload_ctx_ptr->data is NULL" << std::endl;
    return -1;
  }

  // Copy fixed payload data directly to payload context
  memcpy(lc->payload_ctx_ptr->data, lc->current_payload_flush, payload_fixed_size);

  // Handle dynamic data
  struct dar_array src_dars, dst_dars;
  payload_to_dynamic_allocation_roots(kern_header->event_type,
                                      lc->current_payload_flush, lc->payload_ctx_ptr->data,
                                      &src_dars, &dst_dars);

  // Write dynamic data immediately after fixed-size portion of payload
  char *dyn_write_ptr = static_cast<char *>(lc->payload_ctx_ptr->data) + payload_fixed_size;
  char *payload_end = static_cast<char *>(lc->payload_ctx_ptr->data) + lc->payload_ctx_ptr->size;

  for (u32 j = 0; j < src_dars.length; j++)
  {
    if (!src_dars.data[j] || !dst_dars.data[j])
    {
      continue;
    }

    u64 *descriptor_ptr = reinterpret_cast<u64 *>(src_dars.data[j]);
    u64 descriptor = *descriptor_ptr;

    if (descriptor == 0)
    {
      continue; // No dynamic data
    }

    // Parse descriptor: [byte_index:32][byte_length:32]
    u32 byte_index = (descriptor >> 32) & 0xFFFFFFFF;
    u32 byte_length = descriptor & 0xFFFFFFFF;

    // Get destination buffer
    struct flex_buf *dst_field = reinterpret_cast<struct flex_buf *>(dst_dars.data[j]);

    // Convert absolute byte_index to relative index in current_payload_flush
    u32 buffer_start_byte = raw_start_idx * PAYLOAD_BUFFER_ENTRY_SIZE;
    u32 relative_byte_index;

    if (byte_index >= buffer_start_byte)
    {
      // Normal case: no wrap-around
      relative_byte_index = byte_index - buffer_start_byte;
    }
    else
    {
      // Wrap-around case: byte_index is in the wrapped portion
      u32 buffer_end_byte = PAYLOAD_BUFFER_N_ENTRIES_PER_CPU * PAYLOAD_BUFFER_ENTRY_SIZE;
      u32 bytes_before_wrap = buffer_end_byte - buffer_start_byte;
      relative_byte_index = bytes_before_wrap + byte_index;
    }

    // Bounds check for source buffer
    if (relative_byte_index + byte_length > sizeof(lc->current_payload_flush))
    {
      std::cerr << "Error: Buffer overflow in dynamic data processing" << std::endl;
      std::cerr << "  Details:" << std::endl;
      std::cerr << "    - relative_byte_index: " << relative_byte_index << std::endl;
      std::cerr << "    - byte_length: " << byte_length << std::endl;
      std::cerr << "    - buffer size: " << sizeof(lc->current_payload_flush) << std::endl;
      std::cerr << "    - overflow by: " << (relative_byte_index + byte_length - sizeof(lc->current_payload_flush)) << " bytes" << std::endl;
      std::cerr << "    - descriptor: 0x" << std::hex << descriptor << std::dec << std::endl;
      std::cerr << "    - byte_index: " << byte_index << std::endl;
      std::cerr << "    - buffer_start_byte: " << buffer_start_byte << std::endl;
      std::cerr << "    - raw_start_idx: " << raw_start_idx << std::endl;
      std::cerr << "    - PAYLOAD_BUFFER_ENTRY_SIZE: " << PAYLOAD_BUFFER_ENTRY_SIZE << std::endl;
      std::cerr << "    - PAYLOAD_BUFFER_N_ENTRIES_PER_CPU: " << PAYLOAD_BUFFER_N_ENTRIES_PER_CPU << std::endl;

      // Abort copying the field
      dst_field->byte_length = 0;
      dst_field->data = nullptr;
      continue;
    }

    // Bounds check for destination buffer
    if (dyn_write_ptr + byte_length > payload_end)
    {
      std::cerr << "Error: Dynamic data would overflow payload buffer" << std::endl;
      // Fix #2b: Zero the destination field
      dst_field->byte_length = 0;
      dst_field->data = nullptr;
      continue;
    }

    // If we get here but byte_length is 0, also zero the field
    if (byte_length == 0)
    {
      dst_field->byte_length = 0;
      dst_field->data = nullptr;
      continue;
    }

    // Copy dynamic data
    memcpy(dyn_write_ptr, &lc->current_payload_flush[relative_byte_index], byte_length);

    // Set up destination field
    dst_field->byte_length = byte_length;
    dst_field->data = dyn_write_ptr;

    dyn_write_ptr += byte_length;
  }

  // Notify consumer with both header and payload data
  lc->event_cb(lc->header_ctx_ptr, lc->payload_ctx_ptr);

  return 0;
}

/* -------------------------------------------------------------------------- */
/*                    Initialisation and setup (glue code)                    */
/* -------------------------------------------------------------------------- */

struct ConfigItem
{
  u32 key;
  u64 value;
  const char *name;
};

// Public API implementation
extern "C" int initialize(header_ctx *header_ctx_param, payload_ctx *payload_ctx_param,
                          event_callback_t event_callback)
{
  // Validate parameters
  if (!header_ctx_param || !header_ctx_param->data || !payload_ctx_param ||
      !payload_ctx_param->data || !event_callback)
  {
    std::cerr << "Error: Invalid parameters in initialize" << std::endl;
    return -1;
  }

  // Initialize library context
  struct lib_ctx lc = {};
  lc.header_ctx_ptr = header_ctx_param;
  lc.event_cb = event_callback;
  lc.payload_ctx_ptr = payload_ctx_param;

  int err;
  int config_fd;

  ConfigItem configs[] = {
      {CONFIG_DEBUG_ENABLED, static_cast<u64>(env.debug_bpf ? 1 : 0), "debug_enabled"},
      {CONFIG_SYSTEM_BOOT_NS, get_system_boot_ns(), "system_boot_ns"},
  };

  libbpf_set_print(libbpf_print_cb);
  signal(SIGINT, sig_handler);
  signal(SIGTERM, sig_handler);

  lc.skel = bootstrap_bpf__open();
  if (!lc.skel)
  {
    std::cerr << "Failed to open skeleton" << std::endl;
    return 1;
  }

  err = bootstrap_bpf__load(lc.skel);
  if (err)
  {
    std::cerr << "Load failed: " << err << std::endl;
    goto out;
  }

  // Initialize configuration map
  config_fd = bpf_map__fd(lc.skel->maps.config);
  if (config_fd < 0)
  {
    std::cerr << "Failed to get config map fd" << std::endl;
    err = -1;
    goto out;
  }

  // Set basic configuration values
  for (const auto &config : configs)
  {
    err = bpf_map__update_elem(lc.skel->maps.config, &config.key, sizeof(u32),
                               &config.value, sizeof(u64), BPF_ANY);
    if (err)
    {
      std::cerr << "Failed to set " << config.name << ": " << err << std::endl;
      goto out;
    }
  }

  // Get file descriptor for shared array map
  lc.payload_buffer_fd = bpf_map__fd(lc.skel->maps.payload_buffer);
  if (lc.payload_buffer_fd < 0)
  {
    std::cerr << "Failed to get payload_buffer map fd" << std::endl;
    err = -1;
    goto out;
  }

  // Setup kernel-level filtering
  bootstrap_filter__register_skeleton(lc.skel);

  err = bootstrap_bpf__attach(lc.skel);
  if (err)
  {
    std::cerr << "Attach failed: " << err << std::endl;
    goto out;
  }

  lc.rb = ring_buffer__new(
      bpf_map__fd(lc.skel->maps.rb),
      handle_header_flush, &lc, NULL);
  if (!lc.rb)
  {
    std::cerr << "Ring-buffer create failed" << std::endl;
    err = -1;
    goto out;
  }

  while (!exiting)
  {
    err = ring_buffer__poll(lc.rb, 200 /* timeout, ms */);
    if (err == -EINTR)
      err = 0;
    if (err < 0)
    {
      std::cerr << "Poll error " << err << std::endl;
      break;
    }
  }

out:
  ring_buffer__free(lc.rb);
  bootstrap_bpf__destroy(lc.skel);

  return err < 0 ? -err : 0;
}
