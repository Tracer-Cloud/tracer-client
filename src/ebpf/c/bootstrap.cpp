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

// Extended library context to handle per-CPU arrays and 2-layer buffering
struct lib_ctx
{
  // Context structures for the new API
  header_ctx *header_ctx_ptr;
  payload_ctx *payload_ctx_ptr;
  header_callback_t header_cb;
  payload_callback_t payload_cb;

  // Other context
  struct bootstrap_bpf *skel; // BPF skeleton
  struct ring_buffer *rb;     // Ring buffer
  int data_buffer_fd;         // Per-CPU array map fd

  // Per-CPU page buffers for fast access
  void **cpu_pages; // Array of pointers to per-CPU pages
  int n_cpus;       // Number of CPUs

  // Pending payloads tracking
  std::vector<std::vector<pending_payload_info>> pending_payloads; // One vector per CPU
  u8 current_payload_flush[PAYLOAD_FLUSH_MAX_PAGES][PAGE_SIZE];
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
/*                            Payload processing                              */
/* -------------------------------------------------------------------------- */

// Process a single dynamic allocation and convert to flex_buf
static void process_dynamic_allocation(u64 descriptor,
                                       u64 *descriptor_field,
                                       char *kernel_allocation_data,
                                       char **flex_buf_write_ptr,
                                       char *flex_buf_write_end)
{
  // Parse descriptor: [is_final:1][order:16][size:47] for root allocations
  bool is_final = (descriptor >> 63) & 1;
  u64 size = descriptor & 0x7FFFFFFFFFFFULL;

  // For single-node case, we expect is_final to be true
  if (!is_final)
  {
    std::cerr << "Multi-node dynamic allocations not yet supported" << std::endl;
    return;
  }

  // Create flex_buf in user payload buffer
  if (*flex_buf_write_ptr + sizeof(u32) + size > flex_buf_write_end)
  {
    std::cerr << "Not enough space for flex_buf conversion" << std::endl;
    return;
  }

  struct flex_buf *fb = reinterpret_cast<struct flex_buf *>(*flex_buf_write_ptr);
  fb->byte_length = size;
  memcpy(fb->data, kernel_allocation_data, size);

  // Update the descriptor field to point to our flex_buf
  *descriptor_field = reinterpret_cast<u64>(fb);

  // Move write pointer for next flex_buf
  *flex_buf_write_ptr += sizeof(u32) + size;
  // Align to 8-byte boundary
  *flex_buf_write_ptr = reinterpret_cast<char *>((reinterpret_cast<uintptr_t>(*flex_buf_write_ptr) + 7) & ~7);
}

// Process payload data from per-CPU pages
static int handle_payload_flush(struct lib_ctx *lc, u16 cpu)
{
  // Check if we have any pending payloads for this CPU
  if (lc->pending_payloads[cpu].empty())
  {
    return 0;
  }

  u16 first_page = lc->pending_payloads[cpu][0].page_index;
  u16 last_page = lc->pending_payloads[cpu].back().page_index;

  // Calculate upper bound on space needed for complete flush
  u16 page_span = (last_page >= first_page) ? (last_page - first_page + 1) : (last_page + PAYLOAD_BUFFER_N_PAGES - first_page + 1);
  size_t num_payloads = lc->pending_payloads[cpu].size();
  // TODO (when implementing dynamic allocation chain collapse): 4 extra bytes needed per flex_buf
  size_t space_needed_upper_bound = sizeof(struct payload_batch_header) +
                                    (num_payloads * sizeof(struct payload_batch_index_entry)) +
                                    page_span * PAGE_SIZE;

  // Ensure enough space is available upfront
  if (space_needed_upper_bound > lc->payload_ctx_ptr->size)
  {
    // Invoke callback with empty payload to request more space
    lc->payload_ctx_ptr->data->bytes_written = 0;
    lc->payload_ctx_ptr->data->num_payloads = 0;
    lc->payload_cb(lc->payload_ctx_ptr);

    // Verify we now have enough space
    if (space_needed_upper_bound > lc->payload_ctx_ptr->size)
    {
      std::cerr << "Consumer failed to provide sufficient space: needed "
                << space_needed_upper_bound << ", got " << lc->payload_ctx_ptr->size << std::endl;
      return -1;
    }
  }

  // Set up direct buffer layout: header, then index array, then payload data
  char *buffer = reinterpret_cast<char *>(lc->payload_ctx_ptr->data);
  struct payload_batch_header *header = reinterpret_cast<struct payload_batch_header *>(buffer);

  // Index array starts right after the header
  struct payload_batch_index_entry *index_array =
      reinterpret_cast<struct payload_batch_index_entry *>(buffer + sizeof(struct payload_batch_header));

  // Payload data starts after the index array
  char *payload_data_start = buffer + sizeof(struct payload_batch_header) +
                             (num_payloads * sizeof(struct payload_batch_index_entry));

  // Update header to point to the correctly positioned arrays
  header->payload_index = index_array;
  header->payload_data = payload_data_start;
  header->bytes_written = 0;
  header->num_payloads = 0;

  // Loop through all pending payloads for this CPU
  for (size_t i = 0; i < num_payloads; i++)
  {
    const auto &pending = lc->pending_payloads[cpu][i];

    // Calculate the correct page in current_payload_flush buffer
    // Handle wrap-around: if page < first_page, it wrapped around
    u16 buffer_page_index = (pending.page_index < first_page) ? (pending.page_index + PAYLOAD_BUFFER_N_PAGES - first_page) : (pending.page_index - first_page);

    // Calculate source address in current_payload_flush
    void *src_payload = &lc->current_payload_flush[buffer_page_index][pending.page_offset];

    // Get the kernel payload size for this event type
    size_t kernel_payload_size = get_kernel_payload_size(pending.event_type);

    // Get dynamic allocation roots (for future use with dynamic payloads)
    struct dar_array dar = payload_to_dynamic_allocation_roots(pending.event_type, src_payload);

    size_t total_payload_size = kernel_payload_size;

    // Add index entry for this payload
    index_array[header->num_payloads].event_id = pending.event_id;
    index_array[header->num_payloads].event_type = pending.event_type;
    index_array[header->num_payloads].offset = header->bytes_written;

    // Copy the payload data
    void *dst_payload = payload_data_start + header->bytes_written;
    memcpy(dst_payload, src_payload, kernel_payload_size);

    // Process dynamic allocations if any exist
    if (dar.length > 0)
    {
      // Set up flex_buf writing area after the copied payload
      char *flex_buf_write_ptr = static_cast<char *>(dst_payload) + kernel_payload_size;
      char *flex_buf_write_end = reinterpret_cast<char *>(lc->payload_ctx_ptr->data) + lc->payload_ctx_ptr->size;

      // Dynamic allocation data is stored sequentially after the fixed payload in the kernel page
      char *kernel_dynamic_data = static_cast<char *>(src_payload) + kernel_payload_size;

      // Get dynamic allocation roots for the destination payload
      struct dar_array dst_dar = payload_to_dynamic_allocation_roots(pending.event_type, dst_payload);

      // Process each dynamic allocation root
      for (u32 j = 0; j < dst_dar.length; j++)
      {
        // Read descriptor from SOURCE kernel payload (not destination)
        u64 *src_descriptor_field = reinterpret_cast<u64 *>(dar.data[j]);
        u64 descriptor = *src_descriptor_field;

        // Get corresponding destination field
        u64 *dst_descriptor_field = reinterpret_cast<u64 *>(dst_dar.data[j]);

        // Process the dynamic allocation
        process_dynamic_allocation(descriptor, dst_descriptor_field, kernel_dynamic_data, &flex_buf_write_ptr, flex_buf_write_end);

        // Move to next dynamic allocation (based on size from descriptor)
        u64 size = descriptor & 0x7FFFFFFFFFFFULL;
        kernel_dynamic_data += (size + 7) & ~7; // 8-byte aligned
      }

      total_payload_size = flex_buf_write_ptr - static_cast<char *>(dst_payload);
    }

    // Update batch metadata
    header->bytes_written += total_payload_size;
    header->num_payloads++;
  }

  // Final callback to flush everything in payload_ctx->data
  lc->payload_cb(lc->payload_ctx_ptr);

  // Clear pending payloads for this CPU as they've been processed
  lc->pending_payloads[cpu].clear();

  return 0;
}

/* -------------------------------------------------------------------------- */
/*                           Header processing                                */
/* -------------------------------------------------------------------------- */

// Ring‑buffer callback for processing headers, flush initiated by kernel
static int handle_header_flush(void *ctx, void *data, size_t _)
{
  struct lib_ctx *lc = static_cast<struct lib_ctx *>(ctx);
  struct event_header_kernel *kern_header = static_cast<struct event_header_kernel *>(data);

  bool should_skip = bootstrap_filter__should_skip(kern_header);

  // Step 1: Process header if not filtered out
  if (!should_skip)
  {
    // Generate event ID
    u64 event_id = generate_event_id();

    // Copy header data to header_ctx
    *lc->header_ctx_ptr->data = *((struct event_header_user *)kern_header);
    lc->header_ctx_ptr->data->event_id = event_id;

    // For retrieval of payload once flushed
    pending_payload_info info = {
        .event_id = event_id,
        .event_type = kern_header->event_type,
        .page_index = kern_header->payload.page_index,
        .page_offset = kern_header->payload.byte_offset};

    u32 cpu = kern_header->payload.cpu;
    lc->pending_payloads[cpu].push_back(info);
  }

  // Step 2: Check if kernel is signaling payload flush
  u16 cpu_to_flush = kern_header->payload.cpu;
  u16 payload_pages_to_flush = kern_header->payload.flush_signal;

  // Step 3: Copy payload pages from kernel to userspace (time-sensitive operation)
  if (payload_pages_to_flush > 0)
  {
    // Copy payload pages into lib_ctx->current_payload_flush without modification
    // Has to run before any callbacks triggered because of time-sensitivity
    for (u16 i = 0; i < payload_pages_to_flush; i++)
    {
      u32 page_key = (kern_header->payload.page_index + i) % PAYLOAD_BUFFER_N_PAGES;
      int ret = bpf_map_lookup_elem(lc->data_buffer_fd, &page_key, lc->current_payload_flush[i]);
      if (ret < 0)
      {
        std::cerr << "Failed to lookup page " << page_key << " for CPU " << kern_header->payload.cpu << std::endl;
        return -1;
      }
    }
  }

  // Step 4: Notify consumer of new header data (if not filtered)
  if (!should_skip)
  {
    // Notify consumer of new data. The consumer updates ctx in-place, to tell this file where to write data
    lc->header_cb(lc->header_ctx_ptr);
  }

  // Step 5: Process flushed payload data (after header callback to maintain ordering)
  if (payload_pages_to_flush > 0)
  {
    handle_payload_flush(lc, cpu_to_flush);
  }

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
                          header_callback_t header_callback, payload_callback_t payload_callback)
{
  // Get number of CPUs
  int n_cpus = sysconf(_SC_NPROCESSORS_ONLN);
  if (n_cpus < 1)
    n_cpus = 1;

  // Initialize library context
  struct lib_ctx lc = {};
  lc.header_ctx_ptr = header_ctx_param;
  lc.header_cb = header_callback;
  lc.payload_ctx_ptr = payload_ctx_param;
  lc.payload_cb = payload_callback;
  lc.n_cpus = n_cpus;
  lc.pending_payloads.resize(n_cpus);

  int err;
  int config_fd;

  ConfigItem configs[] = {
      {CONFIG_DEBUG_ENABLED, static_cast<u64>(env.debug_bpf ? 1 : 0), "debug_enabled"},
      {CONFIG_SYSTEM_BOOT_NS, get_system_boot_ns(), "system_boot_ns"},
  };

  libbpf_set_print(libbpf_print_cb);
  signal(SIGINT, sig_handler);
  signal(SIGTERM, sig_handler);

  /* ----------------------------------------------------- */
  /* Steps:
   * 1. Open BPF skeleton
   * 2. Load BPF programs
   * 3. Configure runtime parameters
   * 4. Set up per-CPU array maps
   * 5. Attach BPF programs to tracepoints
   * 6. Register ringbuffer callback
   */

  lc.skel = bootstrap_bpf__open();
  if (!lc.skel)
  {
    std::cerr << "C++: failed to open skeleton" << std::endl;
    return 1;
  }

  err = bootstrap_bpf__load(lc.skel);
  if (err)
  {
    std::cerr << "C++: load failed: " << err << std::endl;
    goto out;
  }

  // Initialize configuration map
  config_fd = bpf_map__fd(lc.skel->maps.config);
  if (config_fd < 0)
  {
    std::cerr << "C++: failed to get config map fd" << std::endl;
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
      std::cerr << "C++: failed to set " << config.name << ": " << err << std::endl;
      goto out;
    }
  }

  // Get file descriptor for per-CPU array map
  lc.data_buffer_fd = bpf_map__fd(lc.skel->maps.data_buffer);
  if (lc.data_buffer_fd < 0)
  {
    std::cerr << "C++: failed to get data_buffer map fd" << std::endl;
    err = -1;
    goto out;
  }

  // Setup kernel-level filtering
  bootstrap_filter__register_skeleton(lc.skel);

  err = bootstrap_bpf__attach(lc.skel);
  if (err)
  {
    std::cerr << "C++: attach failed: " << err << std::endl;
    goto out;
  }

  lc.rb = ring_buffer__new(
      bpf_map__fd(lc.skel->maps.rb),
      handle_header_flush, &lc, NULL);
  if (!lc.rb)
  {
    std::cerr << "C++: ring-buffer create failed" << std::endl;
    err = -1;
    goto out;
  }

  /* ----------------------------------------------------- */

  while (!exiting)
  {
    err = ring_buffer__poll(lc.rb, 200 /* timeout, ms */);
    if (err == -EINTR)
      err = 0;
    if (err < 0)
    {
      std::cerr << "C++: poll error " << err << std::endl;
      break;
    }
  }

out:
  ring_buffer__free(lc.rb);
  bootstrap_bpf__destroy(lc.skel);
  free(lc.cpu_pages);
  return err < 0 ? -err : 0;
}
