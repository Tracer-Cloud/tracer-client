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

// Helper to calculate CPU-specific key for the payload_buffer map (userspace version)
// Each CPU gets a contiguous range: CPU N uses keys [N * PAYLOAD_BUFFER_N_PAGES, (N+1) * PAYLOAD_BUFFER_N_PAGES)
static u32 cpu_page_key(u32 page_index, u32 cpu)
{
  return cpu * PAYLOAD_BUFFER_N_PAGES + (page_index % PAYLOAD_BUFFER_N_PAGES);
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
  int payload_buffer_fd;      // Shared array map fd (with manual CPU isolation)

  // Pending payloads tracking
  int n_cpus;                                                      // Number of CPUs
  std::vector<std::vector<pending_payload_info>> pending_payloads; // One vector per CPU
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
/*                            Payload processing                              */
/* -------------------------------------------------------------------------- */

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
    u32 src_offset = buffer_page_index * PAGE_SIZE + pending.page_offset;

    // Calculate source address in current_payload_flush
    void *src_payload = &lc->current_payload_flush[src_offset];

    size_t payload_fixed_size = get_payload_fixed_size(pending.event_type);

    // Add index entry for this payload
    index_array[header->num_payloads].event_id = pending.event_id;
    index_array[header->num_payloads].event_type = pending.event_type;
    index_array[header->num_payloads].offset = header->bytes_written;

    // Copy the fixed-size portion of the payload data
    void *dst_payload = payload_data_start + header->bytes_written;
    memcpy(dst_payload, src_payload, payload_fixed_size);

    header->bytes_written += payload_fixed_size;
    header->num_payloads++;

    // Get dynamic allocation roots
    struct dar_array src_dars, dst_dars;
    payload_to_dynamic_allocation_roots(pending.event_type,
                                        src_payload, dst_payload, &src_dars, &dst_dars);

    src_offset += payload_fixed_size;
    char *dyn_write_ptr = static_cast<char *>(dst_payload) + payload_fixed_size;

    // Process each dynamic allocation root
    for (u32 j = 0; j < src_dars.length; j++)
    {
      u64 *descriptor_ptr = reinterpret_cast<u64 *>(src_dars.data[j]); // root descriptor
      u64 descriptor = *descriptor_ptr;

      std::cerr << "=== Processing dynamic allocation root " << j << " ===" << std::endl;
      std::cerr << "Root descriptor address: " << (void *)descriptor_ptr << std::endl;
      std::cerr << "Root descriptor value: 0x" << std::hex << descriptor << std::dec << std::endl;

      if (descriptor == 0)
      {
        std::cerr << "Root descriptor is null, skipping" << std::endl;
        break;
      }

      // Get corresponding destination field (user-space structure)
      struct flex_buf *dst_field = reinterpret_cast<struct flex_buf *>(dst_dars.data[j]);

      // Track total size and current write position for concatenated data
      u32 total_chain_size = 0;
      char *chain_write_ptr = dyn_write_ptr;
      u32 current_src_offset = src_offset;
      u64 current_descriptor = descriptor;

      std::cerr << "Starting chain traversal at src_offset: " << current_src_offset << std::endl;
      std::cerr << "Chain write pointer: " << (void *)chain_write_ptr << std::endl;

      u32 chain_node_count = 0;

      // Follow the allocation chain and copy data as we go
      while (true)
      {
        chain_node_count++;
        std::cerr << "--- Chain node " << chain_node_count << " ---" << std::endl;
        std::cerr << "Processing descriptor: 0x" << std::hex << current_descriptor << std::dec << std::endl;

        // Parse descriptor: [is_final:1][order:16][size:47]
        bool is_final = (current_descriptor >> 63) & 1;
        u16 order = (current_descriptor >> 47) & 0xFFFF;
        u64 size = current_descriptor & 0x7FFFFFFFFFFFULL;

        std::cerr << "  is_final: " << (is_final ? "true" : "false") << std::endl;
        std::cerr << "  order: " << order << std::endl;
        std::cerr << "  size: " << size << std::endl;
        std::cerr << "  current_src_offset: " << current_src_offset << std::endl;

        if (size > 4096)
        {
          std::cerr << "Allocation size > 4096 not supported, breaking" << std::endl;
          break;
        }

        // Handle page boundary alignment for data
        u32 original_offset = current_src_offset;
        if ((current_src_offset / 4096) != ((current_src_offset + size) / 4096))
        {
          current_src_offset = (current_src_offset + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
          std::cerr << "  Data crosses page boundary, aligned offset " << original_offset << " -> " << current_src_offset << std::endl;
        }

        std::cerr << "  Copying " << size << " bytes from offset " << current_src_offset << " to " << (void *)chain_write_ptr << std::endl;

        // Copy data from this allocation in the chain
        memcpy(chain_write_ptr, &lc->current_payload_flush[current_src_offset], size);

        // Debug: show first few bytes of copied data
        std::cerr << "  First 128 bytes copied: ";
        for (u32 k = 0; k < std::min((u64)128, size); k++)
        {
          char c = chain_write_ptr[k];
          if (c >= 32 && c <= 126)
          {
            std::cerr << c;
          }
          else
          {
            std::cerr << "\\x" << std::hex << (unsigned char)c << std::dec;
          }
        }
        std::cerr << std::endl;

        chain_write_ptr += size;
        total_chain_size += size;
        current_src_offset += size;

        std::cerr << "  Updated total_chain_size: " << total_chain_size << std::endl;
        std::cerr << "  Updated chain_write_ptr: " << (void *)chain_write_ptr << std::endl;
        std::cerr << "  Updated current_src_offset: " << current_src_offset << std::endl;

        if (is_final)
        {
          std::cerr << "  This is the final allocation in the chain" << std::endl;
          break;
        }

        // For chained allocations, next descriptor is 8 bytes before the next data
        // current_src_offset += 8; // Skip over the chain descriptor
        std::cerr << "  Moving to next descriptor at offset: " << current_src_offset << std::endl;

        // Handle page boundary for descriptor
        u32 desc_original_offset = current_src_offset;
        if ((current_src_offset / 4096) != ((current_src_offset + 8) / 4096))
        {
          current_src_offset = (current_src_offset + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
          std::cerr << "  Descriptor crosses page boundary, aligned offset " << desc_original_offset << " -> " << current_src_offset << std::endl;
        }

        if (current_src_offset + 8 > PAYLOAD_FLUSH_MAX_PAGES * PAGE_SIZE)
        {
          std::cerr << "  Chain extends beyond available payload buffer (" << current_src_offset + 8 << " > " << PAYLOAD_FLUSH_MAX_PAGES * PAGE_SIZE << ")" << std::endl;
          break;
        }

        // Read next descriptor
        u64 *next_descriptor_ptr = reinterpret_cast<u64 *>(&lc->current_payload_flush[current_src_offset]);
        current_descriptor = *next_descriptor_ptr;
        current_src_offset += 8; // Move past descriptor

        std::cerr << "  Next descriptor at " << (void *)next_descriptor_ptr << ": 0x" << std::hex << current_descriptor << std::dec << std::endl;

        if (current_descriptor == 0)
        {
          std::cerr << "  Null descriptor found in chain, breaking" << std::endl;
          break;
        }

        if (chain_node_count > 20)
        {
          std::cerr << "  Too many chain nodes (>20), possible infinite loop, breaking" << std::endl;
          break;
        }
      }

      std::cerr << "Chain traversal complete. Total nodes: " << chain_node_count << std::endl;
      std::cerr << "Total chain size: " << total_chain_size << " bytes" << std::endl;

      // Set up destination field with concatenated data
      dst_field->byte_length = total_chain_size;
      dst_field->data = dyn_write_ptr;

      std::cerr << "Set flex_buf: byte_length=" << dst_field->byte_length << ", data=" << (void *)dst_field->data << std::endl;

      // Move write pointer for next dynamic allocation
      dyn_write_ptr += total_chain_size;

      // Update source offset to end of this chain
      src_offset = current_src_offset;

      // Update total payload size to include this dynamic data
      header->bytes_written += total_chain_size;

      std::cerr << "Updated dyn_write_ptr: " << (void *)dyn_write_ptr << std::endl;
      std::cerr << "Updated src_offset: " << src_offset << std::endl;
      std::cerr << "Updated header->bytes_written: " << header->bytes_written << std::endl;
      std::cerr << "=== End dynamic allocation root " << j << " ===" << std::endl
                << std::endl;
    }
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
  if (payload_pages_to_flush > 0 && !lc->pending_payloads[cpu_to_flush].empty())
  {
    // Copy payload pages into lib_ctx->current_payload_flush without modification
    // Has to run before any callbacks triggered because of time-sensitivity
    // std::cerr << "Copying " << payload_pages_to_flush << " pages starting from page " << kern_header->payload.page_index << " for CPU " << kern_header->payload.cpu << std::endl;

    for (u16 i = 0; i < payload_pages_to_flush; i++)
    {
      u32 page_key = cpu_page_key((kern_header->payload.page_index + i) % PAYLOAD_BUFFER_N_PAGES, cpu_to_flush);
      void *dst = &lc->current_payload_flush[i * PAGE_SIZE];
      int err = bpf_map_lookup_elem(lc->payload_buffer_fd, &page_key, dst);

      if (err)
      {
        std::cerr << "lookup failed for page " << page_key
                  << " cpu " << cpu_to_flush << " → " << err << '\n';
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
  if (payload_pages_to_flush > 0 && !lc->pending_payloads[cpu_to_flush].empty())
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

  /* Steps:
   * 1. Open BPF skeleton
   * 2. Load BPF programs
   * 3. Configure runtime parameters
   * 4. Set up shared array map with manual CPU isolation
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

  // Get file descriptor for shared array map
  lc.payload_buffer_fd = bpf_map__fd(lc.skel->maps.payload_buffer);
  if (lc.payload_buffer_fd < 0)
  {
    std::cerr << "C++: failed to get payload_buffer map fd" << std::endl;
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
  return err < 0 ? -err : 0;
}
