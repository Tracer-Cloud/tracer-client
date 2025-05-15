#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <iostream>
#include <vector>
#include "../../../vendor/nlohmann/json.hpp"

// C headers must stay in an extern "C" block to avoid name‑mangling
extern "C"
{
#include "bootstrap.h"
#include "bootstrap_api.h"
}

// ----------------------------------------------
// Constants & Globals
// ----------------------------------------------
constexpr size_t BUFFER_SIZE = 1 * 1024 * 1024; // 1 MiB ring‑buffer
static volatile sig_atomic_t exiting = 0;

// ----------------------------------------------
// Helper utilities
// ----------------------------------------------
static const char *event_type_to_string(event_type t)
{
  switch (t)
  {
  case EVENT__SCHED__SCHED_PROCESS_EXEC:
    return "process_exec";
  case EVENT__SCHED__SCHED_PROCESS_EXIT:
    return "process_exit";
  case EVENT__SYSCALL__SYS_ENTER_OPENAT:
    return "sys_enter_openat";
  case EVENT__SYSCALL__SYS_EXIT_OPENAT:
    return "sys_exit_openat";
  default:
    return "unknown";
  }
}

static void print_event_json(const event *e)
{
  using json = nlohmann::json;
  json j;

  // Common fields
  j["event_type"] = event_type_to_string(e->event_type);
  j["timestamp_ns"] = e->timestamp_ns;
  j["pid"] = e->pid;
  j["ppid"] = e->ppid;
  j["upid"] = e->upid;
  j["uppid"] = e->uppid;

  // Variant payload
  switch (e->event_type)
  {
  case EVENT__SCHED__SCHED_PROCESS_EXEC:
  {
    const auto &p = e->sched__sched_process_exec__payload;
    j["comm"] = p.comm;
    j["argc"] = p.argc;
    std::vector<std::string> argv;
    for (int i = 0; i < p.argc && i < MAX_ARR_LEN; ++i)
      argv.emplace_back(p.argv[i]);
    j["argv"] = argv;
    break;
  }
  case EVENT__SYSCALL__SYS_ENTER_OPENAT:
  {
    const auto &p = e->syscall__sys_enter_openat__payload;
    j["dfd"] = p.dfd;
    j["filename"] = p.filename;
    j["flags"] = p.flags;
    j["mode"] = p.mode;
    break;
  }
  case EVENT__SYSCALL__SYS_EXIT_OPENAT:
  {
    const auto &p = e->syscall__sys_exit_openat__payload;
    j["fd"] = p.fd;
    break;
  }
  default:
    break; // nothing extra to add
  }

  std::cout << j.dump() << '\n';
}

// ----------------------------------------------
// Ring‑buffer consumer callback
// ----------------------------------------------
static void process_events(void *ctx, size_t bytes)
{
  auto *buffer = static_cast<char *>(ctx);
  size_t pos = 0;

  while (pos + sizeof(event) <= bytes)
  {
    const auto *ev = reinterpret_cast<const event *>(buffer + pos);
    print_event_json(ev);
    pos += sizeof(event);
  }

  if (pos < bytes)
    std::fprintf(stderr, "[warn] %zu trailing bytes\n", bytes - pos);
}

static void sig_handler(int) { exiting = 1; }

// ----------------------------------------------
// main()
// ----------------------------------------------
int main()
{
  // Allocate a user‑space buffer that the bootstrap.c helper will fill
  void *buf = std::malloc(BUFFER_SIZE);
  if (!buf)
  {
    std::perror("malloc");
    return EXIT_FAILURE;
  }

  std::signal(SIGINT, sig_handler);
  std::signal(SIGTERM, sig_handler);

  std::cout << "Starting eBPF event logger – press Ctrl+C to stop...\n";

  int err = initialize(buf, BUFFER_SIZE, process_events, buf);

  std::free(buf);
  if (err)
  {
    std::fprintf(stderr, "initialize() failed: %d\n", err);
    return EXIT_FAILURE;
  }
  std::cout << "Exiting cleanly\n";
  return EXIT_SUCCESS;
}
