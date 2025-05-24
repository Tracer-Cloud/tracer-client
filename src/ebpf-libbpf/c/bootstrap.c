#include <signal.h>
#include <stdio.h>
#include <time.h>
#include <string.h>
#include <sys/resource.h>
#include <bpf/libbpf.h>
#include "bootstrap.h"
#include "bootstrap.skel.h"
#include "bootstrap_api.h"

static struct env
{
	bool verbose;
} env;

static int libbpf_print_fn(enum libbpf_print_level level, const char *format, va_list args)
{
	if (level == LIBBPF_DEBUG && !env.verbose)
		return 0;
	return vfprintf(stderr, format, args);
}

static volatile bool exiting = false;

static void sig_handler(int sig)
{
	exiting = true;
}

// Callback type for the library version to use
typedef void (*event_callback_t)(void *context, size_t filled_bytes);

// Structure to hold the library mode context
struct lib_ctx
{
	void *buffer;
	size_t buffer_size;
	event_callback_t callback;
	void *callback_ctx;
	struct bootstrap_bpf *skel;
	struct ring_buffer *rb;
};

// Handle event in standalone mode - prints to stdout
static int handle_event_standalone(void *ctx, void *data, size_t data_sz)
{
	const struct event *e = data;
	struct tm *tm;
	char ts[32];
	time_t t;
	int i;

	/* Get human readable timestamp for display */
	time(&t);
	tm = localtime(&t);
	strftime(ts, sizeof(ts), "%H:%M:%S", tm);

	if (e->event_type == 1) // ProcessEnterType::Finish
	{
		printf("%-8s %-5s %-16s %-7d %-7d [finished] ts: %llu\n", ts, "EXIT", e->comm, e->pid,
					 e->ppid, e->time);
	}
	else // ProcessEnterType::Start
	{
		printf("%-8s %-5s %-16s %-7d %-7d %s ts: %llu\n", ts, "EXEC", e->comm, e->pid,
					 e->ppid, e->file_name, e->time);

		/* Print argv if available */
		if (e->len > 0)
		{
			printf("    argv[%ld]: ", e->len);
			for (i = 0; i < e->len; i++)
			{
				printf("%s ", e->argv[i]);
			}
			printf("\n");
		}
	}

	return 0;
}

// Handle event in library mode - copies to buffer and invokes callback
static int handle_event_lib(void *ctx, void *data, size_t data_sz)
{
	struct lib_ctx *lib_ctx = (struct lib_ctx *)ctx;
	static size_t filled_bytes = 0;

	// Validate data and size
	if (!data || data_sz == 0)
	{
		return 0;
	}

	// Check if the event size makes sense
	if (data_sz != sizeof(struct event))
	{
		fprintf(stderr, "C: Warning: Event size mismatch - got %zu, expected %zu\n",
						data_sz, sizeof(struct event));
	}

	// Don't overflow the buffer
	if (filled_bytes + data_sz > lib_ctx->buffer_size)
	{
		// Buffer is full, trigger callback
		if (filled_bytes > 0)
		{
			lib_ctx->callback(lib_ctx->callback_ctx, filled_bytes);
			filled_bytes = 0;
		}

		// If the event is too large for the buffer, we have to skip it
		if (data_sz > lib_ctx->buffer_size)
		{
			fprintf(stderr, "C: Event too large for buffer (%zu > %zu), skipping\n",
							data_sz, lib_ctx->buffer_size);
			return 0;
		}
	}

	// Copy the event data to the buffer
	memcpy((char *)lib_ctx->buffer + filled_bytes, data, data_sz);
	filled_bytes += data_sz;

	// Always flush immediately
	if (filled_bytes > 0)
	{
		lib_ctx->callback(lib_ctx->callback_ctx, filled_bytes);
		filled_bytes = 0;
	}

	return 0;
}

// Library API function, called by the Rust binding
int initialize(void *buffer, size_t byte_count, event_callback_t callback, void *callback_ctx)
{
	struct lib_ctx ctx = {
			.buffer = buffer,
			.buffer_size = byte_count,
			.callback = callback,
			.callback_ctx = callback_ctx,
			.skel = NULL,
			.rb = NULL};
	int err;

	// Safety: maximum number of events to process in one batch  
	const int max_events_per_poll = 10000;  // Increased from 100
	// Safety: maximum time to run before returning (in seconds)
	const int max_runtime_seconds = 300;     // Increased from 30 to 5 minutes  
	// Safety: maximum number of polling iterations
	const int max_poll_iterations = 50000;   // Increased from 5

	time_t start_time = time(NULL);
	int poll_count = 0;
	int total_events = 0;

	/* Set up libbpf errors and debug info callback */
	libbpf_set_print(libbpf_print_fn);

	/* Load and verify BPF application */
	ctx.skel = bootstrap_bpf__open();
	if (!ctx.skel)
	{
		fprintf(stderr, "Failed to open and load BPF skeleton\n");
		return 1;
	}

	/* Load & verify BPF programs */
	err = bootstrap_bpf__load(ctx.skel);
	if (err)
	{
		fprintf(stderr, "Failed to load and verify BPF skeleton\n");
		goto cleanup;
	}

	/* Attach tracepoints */
	err = bootstrap_bpf__attach(ctx.skel);
	if (err)
	{
		fprintf(stderr, "Failed to attach BPF skeleton\n");
		goto cleanup;
	}

	/* Set up ring buffer polling */
	ctx.rb = ring_buffer__new(bpf_map__fd(ctx.skel->maps.rb), handle_event_lib, &ctx, NULL);
	if (!ctx.rb)
	{
		err = -1;
		fprintf(stderr, "Failed to create ring buffer\n");
		goto cleanup;
	}

	printf("eBPF: Starting event processing loop (max_iterations=%d, max_runtime=%d, max_events=%d)\n", 
	       max_poll_iterations, max_runtime_seconds, max_events_per_poll);

	/* Process events */
	while (!exiting)
	{
		poll_count++;

		// Safety check for maximum iterations
		if (poll_count > max_poll_iterations)
		{
			printf("eBPF: Reached max iterations (%d), exiting\n", max_poll_iterations);
			break;
		}

		// Safety check for maximum runtime
		time_t current_time = time(NULL);
		if (difftime(current_time, start_time) > max_runtime_seconds)
		{
			printf("eBPF: Reached max runtime (%d seconds), exiting\n", max_runtime_seconds);
			break;
		}

		// Poll with short timeout
		err = ring_buffer__poll(ctx.rb, 100 /* timeout, ms */);

		// Count total events processed
		if (err > 0)
		{
			total_events += err;
			printf("eBPF: Processed %d events (total: %d)\n", err, total_events);

			// Safety check for maximum events
			if (total_events > max_events_per_poll)
			{
				printf("eBPF: Reached max events (%d), exiting\n", max_events_per_poll);
				break;
			}
		}

		/* Ctrl-C will cause -EINTR */
		if (err == -EINTR)
		{
			printf("eBPF: Received interrupt signal, exiting\n");
			err = 0;
			break;
		}
		if (err < 0)
		{
			fprintf(stderr, "Error polling ring buffer: %d\n", err);
			break;
		}
	}

	printf("eBPF: Event processing loop finished (poll_count=%d, total_events=%d)\n", 
	       poll_count, total_events);

cleanup:
	/* Clean up */
	ring_buffer__free(ctx.rb);
	bootstrap_bpf__destroy(ctx.skel);

	return err < 0 ? -err : 0;
}

#ifndef LIBRARY_MODE
int main(int argc, char **argv)
{
	struct ring_buffer *rb = NULL;
	struct bootstrap_bpf *skel;
	int err;

	/* Set verbose if argument is passed */
	if (argc > 1 && strcmp(argv[1], "-v") == 0)
		env.verbose = true;

	/* Set up libbpf errors and debug info callback */
	libbpf_set_print(libbpf_print_fn);

	/* Cleaner handling of Ctrl-C */
	signal(SIGINT, sig_handler);
	signal(SIGTERM, sig_handler);

	/* Load and verify BPF application */
	skel = bootstrap_bpf__open();
	if (!skel)
	{
		fprintf(stderr, "Failed to open and load BPF skeleton\n");
		return 1;
	}

	/* Load & verify BPF programs */
	err = bootstrap_bpf__load(skel);
	if (err)
	{
		fprintf(stderr, "Failed to load and verify BPF skeleton\n");
		goto cleanup;
	}

	/* Attach tracepoints */
	err = bootstrap_bpf__attach(skel);
	if (err)
	{
		fprintf(stderr, "Failed to attach BPF skeleton\n");
		goto cleanup;
	}

	/* Set up ring buffer polling */
	rb = ring_buffer__new(bpf_map__fd(skel->maps.rb), handle_event_standalone, NULL, NULL);
	if (!rb)
	{
		err = -1;
		fprintf(stderr, "Failed to create ring buffer\n");
		goto cleanup;
	}

	/* Process events */
	printf("%-8s %-5s %-16s %-7s %-7s %s\n", "TIME", "EVENT", "COMM", "PID", "PPID",
				 "FILENAME/EXIT CODE");

	while (!exiting)
	{
		err = ring_buffer__poll(rb, 100 /* timeout, ms. no impact on latency */);
		/* Ctrl-C will cause -EINTR */
		if (err == -EINTR)
		{
			err = 0;
			break;
		}
		if (err < 0)
		{
			printf("Error polling perf buffer: %d\n", err);
			break;
		}
	}

cleanup:
	/* Clean up */
	ring_buffer__free(rb);
	bootstrap_bpf__destroy(skel);

	return err < 0 ? -err : 0;
}
#endif
