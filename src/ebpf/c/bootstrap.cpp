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
#define PAYLOAD_FLUSH_MAX_PAGES 128 // Should be sufficient for most payloads

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
  std::cout << "[DEBUG] handle_header_flush: ENTRY" << std::endl;
  std::cout.flush();

  if (!ctx)
  {
    std::cout << "[DEBUG] handle_header_flush: ctx is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!data)
  {
    std::cout << "[DEBUG] handle_header_flush: data is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }

  struct lib_ctx *lc = static_cast<struct lib_ctx *>(ctx);
  std::cout << "[DEBUG] handle_header_flush: lib_ctx cast successful" << std::endl;
  std::cout.flush();

  if (!lc->header_ctx_ptr)
  {
    std::cout << "[DEBUG] handle_header_flush: lc->header_ctx_ptr is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!lc->header_ctx_ptr->data)
  {
    std::cout << "[DEBUG] handle_header_flush: lc->header_ctx_ptr->data is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!lc->payload_ctx_ptr)
  {
    std::cout << "[DEBUG] handle_header_flush: lc->payload_ctx_ptr is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!lc->event_cb)
  {
    std::cout << "[DEBUG] handle_header_flush: lc->event_cb is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }

  struct event_header_kernel *kern_header = static_cast<struct event_header_kernel *>(data);
  std::cout << "[DEBUG] handle_header_flush: kernel header cast successful" << std::endl;
  std::cout.flush();

  if (bootstrap_filter__should_skip(kern_header))
  {
    std::cout << "[DEBUG] handle_header_flush: Event filtered, skipping" << std::endl;
    std::cout.flush();
    return 0;
  };

  std::cout << "[DEBUG] handle_header_flush: Event not filtered, processing" << std::endl;
  std::cout.flush();

  // Generate event ID
  u64 event_id = generate_event_id();
  std::cout << "[DEBUG] handle_header_flush: Generated event_id=" << event_id << std::endl;
  std::cout.flush();

  // Copy header data to header_ctx
  std::cout << "[DEBUG] handle_header_flush: Copying header data" << std::endl;
  std::cout.flush();

  *lc->header_ctx_ptr->data = *((struct event_header_user *)kern_header);
  lc->header_ctx_ptr->data->event_id = event_id;

  std::cout << "[DEBUG] handle_header_flush: Header data copied successfully" << std::endl;
  std::cout.flush();

  // Calculate positions within this CPU's buffer
  int raw_start_idx = kern_header->payload.start_index;
  int raw_end_idx = kern_header->payload.end_index;

  std::cout << "[DEBUG] handle_header_flush: Payload indices - start=" << raw_start_idx
            << ", end=" << raw_end_idx << std::endl;
  std::cout.flush();

  int cpu_base = raw_start_idx - raw_start_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
  int start_in_cpu = raw_start_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
  int end_in_cpu = raw_end_idx % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;

  int payload_size = end_in_cpu - start_in_cpu;
  if (start_in_cpu > end_in_cpu)
    payload_size += PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;

  std::cout << "[DEBUG] handle_header_flush: Calculated payload_size=" << payload_size
            << ", cpu_base=" << cpu_base << std::endl;
  std::cout.flush();

  // Copy payload entries to temporary buffer
  std::cout << "[DEBUG] handle_header_flush: Starting payload copy loop" << std::endl;
  std::cout.flush();

  for (u32 i = 0; i < payload_size; i++)
  {
    // Calculate the actual map index with wrap-around
    u32 map_index = cpu_base + (start_in_cpu + i) % PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
    void *dst = &lc->current_payload_flush[i * PAYLOAD_BUFFER_ENTRY_SIZE];

    std::cout << "[DEBUG] handle_header_flush: Copying entry " << i << ", map_index=" << map_index
              << ", dst=" << dst << std::endl;
    std::cout.flush();

    int lookup_result = bpf_map_lookup_elem(lc->payload_buffer_fd, &map_index, dst);
    if (lookup_result != 0)
    {
      std::cout << "[DEBUG] handle_header_flush: bpf_map_lookup_elem failed for index " << map_index
                << ", result=" << lookup_result << std::endl;
      std::cout.flush();
    }
  }

  std::cout << "[DEBUG] handle_header_flush: Payload copy loop completed" << std::endl;
  std::cout.flush();

  if (payload_size <= 0)
  {
    std::cout << "[DEBUG] handle_header_flush: No payload, calling callback with header only" << std::endl;
    std::cout.flush();

    // No payload - just notify consumer of header data
    lc->payload_ctx_ptr->event_id = event_id;
    lc->payload_ctx_ptr->event_type = kern_header->event_type;
    lc->payload_ctx_ptr->data = nullptr;

    std::cout << "[DEBUG] handle_header_flush: About to call event callback (no payload)" << std::endl;
    std::cout.flush();

    lc->event_cb(lc->header_ctx_ptr, lc->payload_ctx_ptr);

    std::cout << "[DEBUG] handle_header_flush: Event callback returned (no payload)" << std::endl;
    std::cout.flush();

    return 0;
  }

  std::cout << "[DEBUG] handle_header_flush: Processing payload data" << std::endl;
  std::cout.flush();

  // Set up payload context for single payload
  lc->payload_ctx_ptr->event_id = event_id;
  lc->payload_ctx_ptr->event_type = kern_header->event_type;

  // Ensure enough space is available for the payload
  size_t payload_fixed_size = get_payload_fixed_size(kern_header->event_type);

  std::cout << "[DEBUG] handle_header_flush: payload_fixed_size=" << payload_fixed_size << std::endl;
  std::cout.flush();

  if (!lc->payload_ctx_ptr->data)
  {
    std::cout << "[DEBUG] handle_header_flush: lc->payload_ctx_ptr->data is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }

  std::cout << "[DEBUG] handle_header_flush: About to copy fixed payload data" << std::endl;
  std::cout.flush();

  // Copy fixed payload data directly to payload context
  memcpy(lc->payload_ctx_ptr->data, lc->current_payload_flush, payload_fixed_size);

  std::cout << "[DEBUG] handle_header_flush: Fixed payload data copied, processing dynamic data" << std::endl;
  std::cout.flush();

  // Handle dynamic data
  struct dar_array src_dars, dst_dars;
  payload_to_dynamic_allocation_roots(kern_header->event_type,
                                      lc->current_payload_flush, lc->payload_ctx_ptr->data,
                                      &src_dars, &dst_dars);

  std::cout << "[DEBUG] handle_header_flush: Dynamic allocation roots processed, src_dars.length="
            << src_dars.length << ", dst_dars.length=" << dst_dars.length << std::endl;
  std::cout.flush();

  // Write dynamic data immediately after fixed-size portion of payload
  char *dyn_write_ptr = static_cast<char *>(lc->payload_ctx_ptr->data) + payload_fixed_size;

  std::cout << "[DEBUG] handle_header_flush: Starting dynamic data processing loop" << std::endl;
  std::cout.flush();

  for (u32 j = 0; j < src_dars.length; j++)
  {
    std::cout << "[DEBUG] handle_header_flush: Processing dynamic data entry " << j << std::endl;
    std::cout.flush();

    if (!src_dars.data[j])
    {
      std::cout << "[DEBUG] handle_header_flush: src_dars.data[" << j << "] is NULL!" << std::endl;
      std::cout.flush();
      continue;
    }
    if (!dst_dars.data[j])
    {
      std::cout << "[DEBUG] handle_header_flush: dst_dars.data[" << j << "] is NULL!" << std::endl;
      std::cout.flush();
      continue;
    }

    u64 *descriptor_ptr = reinterpret_cast<u64 *>(src_dars.data[j]);
    u64 descriptor = *descriptor_ptr;

    std::cout << "[DEBUG] handle_header_flush: Dynamic data entry " << j << ", descriptor=" << descriptor << std::endl;
    std::cout.flush();

    if (descriptor == 0)
    {
      std::cout << "[DEBUG] handle_header_flush: No dynamic data for entry " << j << std::endl;
      std::cout.flush();
      continue; // No dynamic data
    }

    // Parse descriptor: [byte_index:32][byte_length:32]
    u32 byte_index = (descriptor >> 32) & 0xFFFFFFFF;
    u32 byte_length = descriptor & 0xFFFFFFFF;

    std::cout << "[DEBUG] handle_header_flush: Dynamic data " << j << " - byte_index=" << byte_index
              << ", byte_length=" << byte_length << std::endl;
    std::cout.flush();

    // Get destination buffer
    struct flex_buf *dst_field = reinterpret_cast<struct flex_buf *>(dst_dars.data[j]);

    // Convert absolute byte_index to relative index in current_payload_flush
    // byte_index is relative to the payload buffer map, but we copied entries sequentially
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

    std::cout << "[DEBUG] handle_header_flush: Dynamic data " << j << " - relative_byte_index="
              << relative_byte_index << ", dyn_write_ptr=" << (void *)dyn_write_ptr << std::endl;
    std::cout.flush();

    // Bounds check
    if (relative_byte_index + byte_length > sizeof(lc->current_payload_flush))
    {
      std::cout << "[DEBUG] handle_header_flush: ERROR - relative_byte_index + byte_length ("
                << (relative_byte_index + byte_length) << ") > buffer size ("
                << sizeof(lc->current_payload_flush) << ")" << std::endl;
      std::cout.flush();
      continue;
    }

    std::cout << "[DEBUG] handle_header_flush: About to copy dynamic data " << j << std::endl;
    std::cout.flush();

    // Copy dynamic data
    memcpy(dyn_write_ptr, &lc->current_payload_flush[relative_byte_index], byte_length);

    std::cout << "[DEBUG] handle_header_flush: Dynamic data " << j << " copied successfully" << std::endl;
    std::cout.flush();

    // Set up destination field
    dst_field->byte_length = byte_length;
    dst_field->data = dyn_write_ptr;

    dyn_write_ptr += byte_length;
  }

  std::cout << "[DEBUG] handle_header_flush: Dynamic data processing completed, calling callback" << std::endl;
  std::cout.flush();

  // Notify consumer with both header and payload data
  lc->event_cb(lc->header_ctx_ptr, lc->payload_ctx_ptr);

  std::cout << "[DEBUG] handle_header_flush: Event callback returned" << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] handle_header_flush: EXIT" << std::endl;
  std::cout.flush();

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
  std::cout << "[DEBUG] initialize: ENTRY" << std::endl;
  std::cout.flush();

  // Validate parameters
  if (!header_ctx_param)
  {
    std::cout << "[DEBUG] initialize: header_ctx_param is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!header_ctx_param->data)
  {
    std::cout << "[DEBUG] initialize: header_ctx_param->data is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!payload_ctx_param)
  {
    std::cout << "[DEBUG] initialize: payload_ctx_param is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!payload_ctx_param->data)
  {
    std::cout << "[DEBUG] initialize: payload_ctx_param->data is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }
  if (!event_callback)
  {
    std::cout << "[DEBUG] initialize: event_callback is NULL!" << std::endl;
    std::cout.flush();
    return -1;
  }

  std::cout << "[DEBUG] initialize: Parameters validated successfully" << std::endl;
  std::cout.flush();

  // Initialize library context
  struct lib_ctx lc = {};
  lc.header_ctx_ptr = header_ctx_param;
  lc.event_cb = event_callback;
  lc.payload_ctx_ptr = payload_ctx_param;

  std::cout << "[DEBUG] initialize: lib_ctx initialized" << std::endl;
  std::cout.flush();

  int err;
  int config_fd;

  ConfigItem configs[] = {
      {CONFIG_DEBUG_ENABLED, static_cast<u64>(env.debug_bpf ? 1 : 0), "debug_enabled"},
      {CONFIG_SYSTEM_BOOT_NS, get_system_boot_ns(), "system_boot_ns"},
  };

  std::cout << "[DEBUG] initialize: Setting up libbpf and signal handlers" << std::endl;
  std::cout.flush();

  libbpf_set_print(libbpf_print_cb);
  signal(SIGINT, sig_handler);
  signal(SIGTERM, sig_handler);

  /* Steps:
   * 1. Open BPF skeleton
   * 2. Load BPF programs
   * 3. Configure runtime parameters
   * 4. Set up shared array map with manual CPU isolation
   * 5. Attach BPF programs to tracepoints
   * 6. Register ringbuffer callback
   */

  std::cout << "[DEBUG] initialize: Opening BPF skeleton" << std::endl;
  std::cout.flush();

  lc.skel = bootstrap_bpf__open();
  if (!lc.skel)
  {
    std::cerr << "C++: failed to open skeleton" << std::endl;
    return 1;
  }

  std::cout << "[DEBUG] initialize: BPF skeleton opened successfully" << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] initialize: Loading BPF programs" << std::endl;
  std::cout.flush();

  err = bootstrap_bpf__load(lc.skel);
  if (err)
  {
    std::cerr << "C++: load failed: " << err << std::endl;
    goto out;
  }

  std::cout << "[DEBUG] initialize: BPF programs loaded successfully" << std::endl;
  std::cout.flush();

  // Initialize configuration map
  std::cout << "[DEBUG] initialize: Getting config map fd" << std::endl;
  std::cout.flush();

  config_fd = bpf_map__fd(lc.skel->maps.config);
  if (config_fd < 0)
  {
    std::cerr << "C++: failed to get config map fd" << std::endl;
    err = -1;
    goto out;
  }

  std::cout << "[DEBUG] initialize: Config map fd=" << config_fd << std::endl;
  std::cout.flush();

  // Set basic configuration values
  std::cout << "[DEBUG] initialize: Setting configuration values" << std::endl;
  std::cout.flush();

  for (const auto &config : configs)
  {
    std::cout << "[DEBUG] initialize: Setting config " << config.name << "=" << config.value << std::endl;
    std::cout.flush();

    err = bpf_map__update_elem(lc.skel->maps.config, &config.key, sizeof(u32),
                               &config.value, sizeof(u64), BPF_ANY);
    if (err)
    {
      std::cerr << "C++: failed to set " << config.name << ": " << err << std::endl;
      goto out;
    }
  }

  std::cout << "[DEBUG] initialize: Configuration values set successfully" << std::endl;
  std::cout.flush();

  // Get file descriptor for shared array map
  std::cout << "[DEBUG] initialize: Getting payload buffer map fd" << std::endl;
  std::cout.flush();

  lc.payload_buffer_fd = bpf_map__fd(lc.skel->maps.payload_buffer);
  if (lc.payload_buffer_fd < 0)
  {
    std::cerr << "C++: failed to get payload_buffer map fd" << std::endl;
    err = -1;
    goto out;
  }

  std::cout << "[DEBUG] initialize: Payload buffer map fd=" << lc.payload_buffer_fd << std::endl;
  std::cout.flush();

  // Setup kernel-level filtering
  std::cout << "[DEBUG] initialize: Setting up kernel-level filtering" << std::endl;
  std::cout.flush();

  bootstrap_filter__register_skeleton(lc.skel);

  std::cout << "[DEBUG] initialize: Attaching BPF programs" << std::endl;
  std::cout.flush();

  err = bootstrap_bpf__attach(lc.skel);
  if (err)
  {
    std::cerr << "C++: attach failed: " << err << std::endl;
    goto out;
  }

  std::cout << "[DEBUG] initialize: BPF programs attached successfully" << std::endl;
  std::cout.flush();

  std::cout << "[DEBUG] initialize: Creating ring buffer" << std::endl;
  std::cout.flush();

  lc.rb = ring_buffer__new(
      bpf_map__fd(lc.skel->maps.rb),
      handle_header_flush, &lc, NULL);
  if (!lc.rb)
  {
    std::cerr << "C++: ring-buffer create failed" << std::endl;
    err = -1;
    goto out;
  }

  std::cout << "[DEBUG] initialize: Ring buffer created successfully, entering poll loop" << std::endl;
  std::cout.flush();

  /* ----------------------------------------------------- */

  while (!exiting)
  {
    std::cout << "[DEBUG] initialize: Polling ring buffer..." << std::endl;
    std::cout.flush();

    err = ring_buffer__poll(lc.rb, 200 /* timeout, ms */);
    if (err == -EINTR)
      err = 0;
    if (err < 0)
    {
      std::cerr << "C++: poll error " << err << std::endl;
      break;
    }

    std::cout << "[DEBUG] initialize: Poll returned, err=" << err << std::endl;
    std::cout.flush();
  }

  std::cout << "[DEBUG] initialize: Exiting poll loop" << std::endl;
  std::cout.flush();

out:
  std::cout << "[DEBUG] initialize: Cleanup phase" << std::endl;
  std::cout.flush();

  ring_buffer__free(lc.rb);
  bootstrap_bpf__destroy(lc.skel);

  std::cout << "[DEBUG] initialize: EXIT with err=" << err << std::endl;
  std::cout.flush();

  return err < 0 ? -err : 0;
}
