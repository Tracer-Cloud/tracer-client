#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>
#include <sys/stat.h>
#include <errno.h>

#include <bpf/libbpf.h>

#include "bootstrap.h"
#include "bootstrap.skel.h"
#include "bootstrap_api.h"

#ifndef likely
#define likely(x) __builtin_expect(!!(x), 1)
#endif
#ifndef unlikely
#define unlikely(x) __builtin_expect(!!(x), 0)
#endif

/* Time calibration constants */
#define RECALIBRATION_INTERVAL_NS (60ULL * 1000000000) /* 60 seconds in ns */

static struct env
{
	bool verbose;
	bool debug_bpf; // Propagate to .rodata
} env = {
	.verbose = false,
	.debug_bpf = false,
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

// Ring‑buffer callback
typedef void (*event_callback_t)(void *ctx, size_t bytes);

struct lib_ctx
{
	void *buffer;
	size_t buf_sz;
	size_t filled; // Running fill level
	event_callback_t cb;
	void *cb_ctx;
	struct bootstrap_bpf *skel;
	struct ring_buffer *rb;
};

// Copies from ringBuffer to external buffer and invokes callback
static int handle_event(void *ctx, void *data, size_t data_sz)
{
	struct lib_ctx *lc = ctx;
	struct event *e = data; // Cast data to our event struct

	if (unlikely(data_sz != sizeof(struct event)))
	{
		fprintf(stderr, "C: size mismatch (%zu!=%zu)\n",
				data_sz, sizeof(struct event));
		return 0;
	}

	// Flush if no room
	if (lc->filled + data_sz > lc->buf_sz)
	{
		if (lc->filled)
			lc->cb(lc->cb_ctx, lc->filled);
		lc->filled = 0;
	}

	memcpy((char *)lc->buffer + lc->filled, data, data_sz);
	lc->filled += data_sz;

	// Immediate flush
	lc->cb(lc->cb_ctx, lc->filled);
	lc->filled = 0;

	return 0;
}

// Public API
int initialize(void *buffer, size_t byte_cnt,
			   event_callback_t cb, void *cb_ctx)
{
	struct lib_ctx lc = {
		.buffer = buffer,
		.buf_sz = byte_cnt,
		.filled = 0,
		.cb = cb,
		.cb_ctx = cb_ctx,
		.skel = NULL,
		.rb = NULL,
	};
	int err;

	libbpf_set_print(libbpf_print_cb);
	signal(SIGINT, sig_handler);
	signal(SIGTERM, sig_handler);

	/* ----------------------------------------------------- */

	lc.skel = bootstrap_bpf__open();
	if (!lc.skel)
	{
		fprintf(stderr, "C: failed to open skeleton\n");
		return 1;
	}

	// Propagate runtime knobs into .rodata
	lc.skel->rodata->debug_enabled = env.debug_bpf;
	lc.skel->rodata->system_boot_ns = get_system_boot_ns();

	err = bootstrap_bpf__load(lc.skel);
	if (err)
	{
		fprintf(stderr, "C: load failed: %d\n", err);
		goto out;
	}
	err = bootstrap_bpf__attach(lc.skel);
	if (err)
	{
		fprintf(stderr, "C: attach failed: %d\n", err);
		goto out;
	}

	lc.rb = ring_buffer__new(
		bpf_map__fd(lc.skel->maps.rb),
		handle_event, &lc, NULL);
	if (!lc.rb)
	{
		fprintf(stderr, "C: ring-buffer create failed\n");
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
			fprintf(stderr, "C: poll error %d\n", err);
			break;
		}
	}

out:
	ring_buffer__free(lc.rb);
	bootstrap_bpf__destroy(lc.skel);
	return err < 0 ? -err : 0;
}