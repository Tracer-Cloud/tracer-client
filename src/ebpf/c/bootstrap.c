/*  Pure-C port of bootstrap.cpp – provides initialize() for FFI users  */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h> /* still needed for sig_atomic_t */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include <bpf/bpf.h>
#include <bpf/libbpf.h>

#include "bootstrap.gen.h"
#include "bootstrap.skel.h"
#include "bootstrap-api.h"
#include "bootstrap-filter.h"

/* -------------------------------------------------------------------------- */
/*  Compile-time helpers                                                      */
/* -------------------------------------------------------------------------- */

#ifndef likely
#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)
#endif

#define FLUSH_MAX_BYTES (64 * 1024) /* payload scratch pad */

/* -------------------------------------------------------------------------- */
/*  Simple environment flags (–v / –d reserved for future)                   */
/* -------------------------------------------------------------------------- */
struct env
{
    bool verbose;
    bool debug_bpf;
} env = {false, false};

/* -------------------------------------------------------------------------- */
/*  Event-ID generator – deterministic within process                         */
/* -------------------------------------------------------------------------- */
static uint64_t generate_event_id(void)
{
    static uint64_t base = 0, counter = 0;

    if (unlikely(!base))
    {
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        base = ((uint64_t)ts.tv_sec << 32) ^ (uint64_t)ts.tv_nsec;
    }
    return base + ++counter;
}

/* -------------------------------------------------------------------------- */
/*  Library context – owned by initialize()                                   */
/* -------------------------------------------------------------------------- */
struct lib_ctx
{
    /* user-supplied */
    header_ctx *hdr;
    payload_ctx *pl;
    event_callback_t cb;

    /* eBPF plumbing */
    struct bootstrap_bpf *skel;
    struct ring_buffer *rb;
    int payload_fd;

    /* scratch */
    uint8_t flush_buf[FLUSH_MAX_BYTES];
};

/* -------------------------------------------------------------------------- */
/*  Misc helpers                                                              */
/* -------------------------------------------------------------------------- */
static uint64_t get_system_boot_ns(void)
{
    struct timespec rt, mono;
    clock_gettime(CLOCK_REALTIME, &rt);
    clock_gettime(CLOCK_MONOTONIC, &mono);

    uint64_t rt_ns = (uint64_t)rt.tv_sec * 1000000000ULL + rt.tv_nsec;
    uint64_t mono_ns = (uint64_t)mono.tv_sec * 1000000000ULL + mono.tv_nsec;
    return rt_ns - mono_ns;
}

static int libbpf_log(enum libbpf_print_level lvl,
                      const char *fmt, va_list ap)
{
    if (lvl == LIBBPF_DEBUG && !env.verbose)
        return 0;
    return vfprintf(stderr, fmt, ap);
}

/* set via shutdown() – no more in-library signal handlers */
static volatile sig_atomic_t exiting = 0;

void shutdown(void)
{
    exiting = 1;
}

/* -------------------------------------------------------------------------- */
/*  Ring-buffer callback – translates kernel ➜ user                           */
/* -------------------------------------------------------------------------- */
static int handle_header_flush(void *ctx, void *data, size_t data_sz __attribute__((unused)))
{
    struct lib_ctx *lc = ctx;
    struct event_header_kernel *kh = data;

    if (!lc || !kh)
        return 0;
    if (bootstrap_filter__should_skip(kh))
        return 0;

    /* ---------- header ----------------------------------------------------- */
    uint64_t eid = generate_event_id();
    memcpy(lc->hdr->data, kh, sizeof(*kh));
    lc->hdr->data->event_id = eid;
    lc->hdr->data->payload = NULL;

    /* ---------- payload indices in the big per-CPU array ------------------- */
    uint32_t raw_start = kh->payload.start_index;
    uint32_t raw_end = kh->payload.end_index;

    const uint32_t per_cpu = PAYLOAD_BUFFER_N_ENTRIES_PER_CPU;
    uint32_t cpu_base = raw_start - (raw_start % per_cpu);

    uint32_t start_in_cpu = raw_start % per_cpu;
    uint32_t end_in_cpu = raw_end % per_cpu;
    uint32_t entries = (end_in_cpu + per_cpu - start_in_cpu) % per_cpu;

    /* ---------- fast path: header-only event ------------------------------- */
    if (entries == 0)
    {
        lc->pl->event_id = eid;
        lc->pl->event_type = kh->event_type;
        lc->pl->data = NULL;
        lc->cb(lc->hdr, lc->pl);
        return 0;
    }

    /* ---------- copy payload pages into scratch --------------------------- */
    const size_t ENTRY_SZ = PAYLOAD_BUFFER_ENTRY_SIZE;
    for (uint32_t i = 0; i < entries; ++i)
    {
        size_t dst_off = (size_t)i * ENTRY_SZ;
        if (dst_off >= sizeof(lc->flush_buf))
            break; /* safety */

        uint32_t idx = cpu_base + (start_in_cpu + i) % per_cpu;
        if (bpf_map_lookup_elem(lc->payload_fd, &idx,
                                lc->flush_buf + dst_off))
            fprintf(stderr, "bpf_map_lookup_elem(%u) failed: %s\n",
                    idx, strerror(errno));
    }

    /* ---------- fixed part ------------------------------------------------- */
    lc->pl->event_id = eid;
    lc->pl->event_type = kh->event_type;

    size_t fixed = get_payload_fixed_size(kh->event_type);
    memcpy(lc->pl->data, lc->flush_buf, fixed);

    /* ---------- dynamic attributes  --------------------------------------- */
    struct dar_array src_roots, dst_roots;
    payload_to_dynamic_allocation_roots(kh->event_type,
                                        lc->flush_buf,
                                        lc->pl->data,
                                        &src_roots,
                                        &dst_roots);

    uint8_t *dyn_write = (uint8_t *)lc->pl->data + fixed;
    uint8_t *dyn_end = (uint8_t *)lc->pl->data + lc->pl->size;

    uint32_t bytes_per_cpu = per_cpu * ENTRY_SZ;
    uint32_t buffer_start = raw_start * ENTRY_SZ;

    for (uint32_t i = 0; i < src_roots.length; ++i)
    {
        uint64_t desc = *(uint64_t *)src_roots.data[i];
        if (!desc)
        { /* field absent */
            ((struct flex_buf *)dst_roots.data[i])->byte_length = 0;
            ((struct flex_buf *)dst_roots.data[i])->data = NULL;
            continue;
        }

        uint32_t byte_idx = (uint32_t)(desc >> 32);
        uint32_t byte_len = (uint32_t)(desc & 0xFFFFFFFFu);

        uint32_t rel_idx =
            (byte_idx + bytes_per_cpu - buffer_start) % bytes_per_cpu;

        if (!byte_len ||
            rel_idx + byte_len > sizeof(lc->flush_buf) ||
            dyn_write + byte_len > dyn_end)
        {
            // Error handling
            ((struct flex_buf *)dst_roots.data[i])->byte_length = 0;
            ((struct flex_buf *)dst_roots.data[i])->data = NULL;
            continue;
        }

        memcpy(dyn_write, lc->flush_buf + rel_idx, byte_len);

        ((struct flex_buf *)dst_roots.data[i])->byte_length = byte_len;
        ((struct flex_buf *)dst_roots.data[i])->data = (char *)dyn_write;
        dyn_write += byte_len;
    }

    lc->cb(lc->hdr, lc->pl);
    return 0;
}

/* -------------------------------------------------------------------------- */
/*  initialise / public API                                                   */
/* -------------------------------------------------------------------------- */
struct cfg_item
{
    u32 key;
    u64 val;
    const char *name;
};

int initialize(header_ctx *hdr,
               payload_ctx *pl,
               event_callback_t cb)
{
    if (!hdr || !hdr->data || !pl || !pl->data || !cb)
        return -EINVAL;

    struct lib_ctx *lc = calloc(1, sizeof(*lc));
    if (!lc)
        return -ENOMEM;

    lc->hdr = hdr;
    lc->pl = pl;
    lc->cb = cb;

    libbpf_set_print(libbpf_log);

    lc->skel = bootstrap_bpf__open();
    if (!lc->skel)
    {
        fprintf(stderr, "bootstrap_bpf__open() failed\n");
        goto err;
    }

    if (bootstrap_bpf__load(lc->skel))
    {
        perror("bootstrap_bpf__load");
        goto err;
    }

    /* basic config */
    {
        struct cfg_item cfgs[] = {
            {CONFIG_DEBUG_ENABLED, (u64)(env.debug_bpf), "debug_enabled"},
            {CONFIG_SYSTEM_BOOT_NS, get_system_boot_ns(), "system_boot_ns"},
        };

        const int cfg_fd = bpf_map__fd(lc->skel->maps.config);
        if (cfg_fd < 0)
        {
            perror("config fd");
            goto err;
        }

        for (size_t i = 0; i < sizeof(cfgs) / sizeof(cfgs[0]); ++i)
            if (bpf_map_update_elem(cfg_fd, &cfgs[i].key,
                                    &cfgs[i].val, BPF_ANY))
            {
                fprintf(stderr, "config[%s] update failed: %s\n",
                        cfgs[i].name, strerror(errno));
                goto err;
            }
    }

    bootstrap_filter__register_skeleton(lc->skel);

    if (bootstrap_bpf__attach(lc->skel))
    {
        perror("bootstrap_bpf__attach");
        goto err;
    }

    lc->payload_fd = bpf_map__fd(lc->skel->maps.payload_buffer);
    if (lc->payload_fd < 0)
    {
        perror("payload_buffer fd");
        goto err;
    }

    lc->rb = ring_buffer__new(bpf_map__fd(lc->skel->maps.rb),
                              handle_header_flush, lc, NULL);
    if (!lc->rb)
    {
        fprintf(stderr, "ring_buffer__new failed\n");
        goto err;
    }

    while (!exiting)
    {
        int r = ring_buffer__poll(lc->rb, 200 /* ms */);
        if (r == -EINTR)
            continue;
        if (r < 0)
        {
            fprintf(stderr, "ring_buffer__poll: %d\n", r);
            break;
        }
    }

    ring_buffer__free(lc->rb);
    bootstrap_bpf__destroy(lc->skel);
    free(lc);
    return 0;

err:
    if (lc->rb)
        ring_buffer__free(lc->rb);
    if (lc->skel)
        bootstrap_bpf__destroy(lc->skel);
    free(lc);
    return -1;
}
