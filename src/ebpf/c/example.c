#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "bootstrap.h"
#include "bootstrap_api.h"

// Buffer size for events
#define BUFFER_SIZE (1024 * 1024) // 1 MB

static volatile bool exiting = false;

static void sig_handler(int sig)
{
  exiting = true;
}

// Format and print event as JSON
static void print_event_json(const struct event *e)
{
  char timestamp[64];
  struct tm *tm;
  time_t event_time = e->timestamp_ns / 1000000000;

  tm = localtime(&event_time);
  strftime(timestamp, sizeof(timestamp), "%Y-%m-%d %H:%M:%S", tm);

  printf("{");
  printf("\"event_type\":\"%s\",", e->event_type == EVENT__SCHED__SCHED_PROCESS_EXEC ? "process_exec" : "process_exit");
  printf("\"timestamp\":\"%s.%09llu\",", timestamp, e->timestamp_ns % 1000000000);
  printf("\"pid\":%u,", e->pid);
  printf("\"ppid\":%u", e->ppid);

  if (e->event_type == EVENT__SCHED__SCHED_PROCESS_EXEC)
  {
    printf(",\"comm\":\"%s\",", e->sched__sched_process_exec__payload.comm);
    printf("\"argc\":%u", e->sched__sched_process_exec__payload.argc);

    if (e->sched__sched_process_exec__payload.argc > 0)
    {
      printf(",\"argv\":[");
      for (int i = 0; i < e->sched__sched_process_exec__payload.argc && i < MAX_ARR_LEN; i++)
      {
        printf("%s\"%s\"", i > 0 ? "," : "", e->sched__sched_process_exec__payload.argv[i]);
      }
      printf("]");
    }
  }

  printf("}\n");
}

// Process events from the buffer
static void process_events(void *ctx, size_t bytes)
{
  void *buffer = ctx;
  size_t pos = 0;

  // Process each complete event in the buffer
  while (pos + sizeof(struct event) <= bytes)
  {
    struct event *e = (struct event *)(buffer + pos);
    print_event_json(e);
    pos += sizeof(struct event);
  }

  // If there was a partial event, warn about it
  if (pos < bytes)
  {
    fprintf(stderr, "Warning: %zu trailing bytes in buffer\n", bytes - pos);
  }
}

int main(int argc, char **argv)
{
  void *buffer;
  int err;

  // Set up signal handlers
  signal(SIGINT, sig_handler);
  signal(SIGTERM, sig_handler);

  // Allocate buffer for events
  buffer = malloc(BUFFER_SIZE);
  if (!buffer)
  {
    fprintf(stderr, "Failed to allocate buffer\n");
    return 1;
  }

  printf("Starting eBPF event logging...\n");
  printf("Press Ctrl+C to exit\n");

  // Start the tracing
  err = initialize(buffer, BUFFER_SIZE, process_events, buffer);

  // Cleanup
  free(buffer);

  if (err)
  {
    fprintf(stderr, "Error: %d\n", err);
    return 1;
  }

  printf("Exited cleanly\n");
  return 0;
}