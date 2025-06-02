#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>
#include <iostream>
#include <cstring>
#include <vector>
#include <random>

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

#define FLUSH_MAX_BYTES (64 * 1024) // 64KB

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
  u8 current_payload_flush[FLUSH_MAX_BYTES];
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
static int handle_header_flush(void *ctx, void *data, size_t /*unused*/)
{
  /* -------- basic validation -------------------------------------------------- */
  if (!ctx || !data)
  {
    std::cerr << "Error: Invalid context or data in handle_header_flush\n";
    return -1;
  }

  auto *lc = static_cast<lib_ctx *>(ctx);

  if (!lc->header_ctx_ptr || !lc->header_ctx_ptr->data ||
      !lc->payload_ctx_ptr || !lc->event_cb)
  {
    std::cerr << "Error: Invalid library context in handle_header_flush\n";
    return -1;
  }

  auto *kern_header = static_cast<event_header_kernel *>(data);

  /* Skip events filtered in user space */
  if (bootstrap_filter__should_skip(kern_header))
    return 0;

  /* -------- header handling ---------------------------------------------------- */
  const u64 event_id = generate_event_id();
  *lc->header_ctx_ptr->data = *reinterpret_cast<event_header_user *>(kern_header);
  lc->header_ctx_ptr->data->event_id = event_id;

  /* -------- payload window calculation ---------------------------------------- */
  const u32 raw_start = kern_header->payload.start_index;
  const u32 raw_end = kern_header->payload.end_index;

  const u32 entries_per_cpu = PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
  const u32 cpu_base = raw_start - raw_start % entries_per_cpu;

  const u32 start_in_cpu = raw_start % entries_per_cpu;
  const u32 end_in_cpu = raw_end % entries_per_cpu;

  u32 payload_entries =
      (end_in_cpu + entries_per_cpu - start_in_cpu) % entries_per_cpu;

  /* -------- copy payload entries into temporary buffer ------------------------ */
  const size_t tmp_capacity = sizeof(lc->current_payload_flush);
  const size_t entry_size = PAYLOAD_BUFFER_ENTRY_SIZE;

  for (u32 i = 0; i < payload_entries; ++i)
  {
    const size_t dst_off = i * entry_size;
    if (dst_off >= tmp_capacity)
    { // hard safety stop
      std::cerr << "Error: Payload copy would overflow buffer at index "
                << i << '\n';
      break;
    }

    const u32 map_index = cpu_base + (start_in_cpu + i) % entries_per_cpu;
    void *dst = lc->current_payload_flush + dst_off;

    if (bpf_map_lookup_elem(lc->payload_buffer_fd, &map_index, dst) != 0)
      std::cerr << "Error: bpf_map_lookup_elem failed for index "
                << map_index << '\n';
  }

  /* -------- fast path: header-only event -------------------------------------- */
  if (payload_entries == 0)
  {
    lc->payload_ctx_ptr->event_id = event_id;
    lc->payload_ctx_ptr->event_type = kern_header->event_type;
    lc->payload_ctx_ptr->data = nullptr;
    lc->event_cb(lc->header_ctx_ptr, lc->payload_ctx_ptr);
    return 0;
  }

  /* -------- payload context initialisation ------------------------------------ */
  lc->payload_ctx_ptr->event_id = event_id;
  lc->payload_ctx_ptr->event_type = kern_header->event_type;

  if (!lc->payload_ctx_ptr->data)
  {
    std::cerr << "Error: payload_ctx_ptr->data is NULL\n";
    return -1;
  }

  const size_t fixed_sz = get_payload_fixed_size(kern_header->event_type);
  std::memcpy(lc->payload_ctx_ptr->data,
              lc->current_payload_flush,
              fixed_sz);

  /* -------- dynamic fields ---------------------------------------------------- */
  dar_array src, dst;
  payload_to_dynamic_allocation_roots(kern_header->event_type,
                                      lc->current_payload_flush,
                                      lc->payload_ctx_ptr->data,
                                      &src, &dst);

  u8 *dyn_write = static_cast<u8 *>(lc->payload_ctx_ptr->data) + fixed_sz;
  u8 *dyn_end = static_cast<u8 *>(lc->payload_ctx_ptr->data) +
                lc->payload_ctx_ptr->size;

  const u32 bytes_per_cpu = entries_per_cpu * entry_size;
  const u32 buffer_start_byte = raw_start * entry_size;

  for (u32 j = 0; j < src.length; ++j)
  {
    if (!src.data[j] || !dst.data[j])
      continue;

    const u64 desc = *reinterpret_cast<u64 *>(src.data[j]);
    if (desc == 0) // field absent
      continue;

    const u32 byte_index = desc >> 32;
    const u32 byte_length = desc & 0xFFFFFFFFu;

    auto *dst_field = reinterpret_cast<flex_buf *>(dst.data[j]);

    /* convert absolute index to index inside current flush buffer */
    const u32 rel_idx =
        (byte_index + bytes_per_cpu - buffer_start_byte) % bytes_per_cpu;

    /* validation */
    if (byte_length == 0 ||
        rel_idx + byte_length > tmp_capacity ||
        dyn_write + byte_length > dyn_end)
    {
      // TODO: investigate cases that end up here (shouldn't happen but it does)
      dst_field->byte_length = 0;
      dst_field->data = nullptr;
      continue;
    }

    /* copy & patch */
    std::memcpy(dyn_write,
                lc->current_payload_flush + rel_idx,
                byte_length);

    dst_field->byte_length = byte_length;
    dst_field->data = reinterpret_cast<char *>(dyn_write);
    dyn_write += byte_length;
  }

  /* -------- deliver event to user code --------------------------------------- */
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
