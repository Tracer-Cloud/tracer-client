#ifndef BOOTSTRAP_FILTER_HPP
#define BOOTSTRAP_FILTER_HPP

#include <vector>
#include <regex>
#include <iostream>
#include <fstream>
#include <algorithm>
#include <unistd.h>
#include <unordered_set>

#include <bpf/libbpf.h>
#include "bootstrap.gen.h"
#include "bootstrap.skel.h"

typedef unsigned long long u64;
typedef unsigned int u32;

// Global variables
static struct bootstrap_bpf *g_skel = nullptr;
static std::unordered_set<u32> g_blacklisted_pids = {0, 1, 2, static_cast<u32>(getpid())}; // Initialize with kernel/init threads and self
static std::unordered_set<u32> g_whitelisted_pids;                                         // Track known safe PIDs
static std::vector<u32> g_kernel_blacklist_subset;                                         // Smaller subset

/* Check if a process should be blacklisted based on an event */
static bool should_blacklist_process(struct event_header_kernel *e)
{
  // Check if comm matches our blacklist pattern
  std::regex pattern("cursor|vscode|iterm|git|sshd|ps", std::regex_constants::icase);
  if (std::regex_search(e->comm, pattern))
  {
    return true;
  }

  // Also check /proc/$pid/cmdline
  std::string cmdline_path = "/proc/" + std::to_string(e->pid) + "/cmdline";
  std::ifstream cmdline_file(cmdline_path);
  if (cmdline_file.is_open())
  {
    std::string cmdline;
    std::getline(cmdline_file, cmdline);

    // Replace null bytes with spaces
    for (char &c : cmdline)
    {
      if (c == '\0')
        c = ' ';
    }

    // Check if cmdline matches our pattern
    if (std::regex_search(cmdline, pattern))
    {
      return true;
    }
    cmdline_file.close();
  }

  return false;
}

/* Maybe update kernel map with blacklisted PIDs */
static void maybe_update_kernel_blacklist() __attribute__((unused));
static void maybe_update_kernel_blacklist()
{
  if (!g_skel || !g_skel->maps.config)
  {
    std::cerr << "Cannot update blacklist: invalid skeleton or map" << std::endl;
    return;
  }

  // Convert set to vector for easier indexing
  std::vector<u32>
      tgids(g_blacklisted_pids.begin(), g_blacklisted_pids.end());

  // Sort PIDs by value (smaller PIDs first as they are likely older processes)
  std::sort(tgids.begin(), tgids.end());

  // Limit to MAX_BLACKLIST_ENTRIES
  int num_entries = std::min(static_cast<size_t>(MAX_BLACKLIST_ENTRIES), tgids.size());
  std::vector<u32> subset(tgids.begin(), tgids.begin() + num_entries);

  // Check if the subset has changed since the last update
  if (subset == g_kernel_blacklist_subset)
  {
    return; // No changes, don't update kernel map
  }

  // Store the new subset for future comparison
  g_kernel_blacklist_subset = subset;

  // Update blacklist entries and clear any remaining ones in a single loop
  for (int i = 0; i < MAX_BLACKLIST_ENTRIES; i++)
  {
    u32 key = CONFIG_PID_BLACKLIST_0 + i;
    u64 value = (i < num_entries) ? subset[i] : 0; // Use 0 (invalid PID) to disable unused entries

    int err = bpf_map__update_elem(g_skel->maps.config, &key, sizeof(u32),
                                   &value, sizeof(u64), BPF_ANY);
    if (err)
    {
      std::cerr << "Failed to update blacklist entry " << i << ": " << err << std::endl;
    }
  }
}

/* Check if an event should be processed or filtered out */
static bool bootstrap_filter__should_skip(struct event_header_kernel *e)
{
  // If process exec event, re-evaluate blacklisting
  if (e->event_type == event_type_sched_sched_process_exec)
  {
    // Remove from both lists to force re-evaluation (pid reuse)
    g_blacklisted_pids.erase(e->pid);
    g_whitelisted_pids.erase(e->pid);
  }

  // If PID is not yet tracked
  if (g_blacklisted_pids.find(e->pid) == g_blacklisted_pids.end() &&
      g_whitelisted_pids.find(e->pid) == g_whitelisted_pids.end())
  {
    // Determine if process should be blacklisted
    if (should_blacklist_process(e))
    {
      g_blacklisted_pids.insert(e->pid);
    }
    else
    {
      g_whitelisted_pids.insert(e->pid);
    }
  }

  if (e->event_type == event_type_sched_sched_process_exit)
  {
    // Remove PID from both blacklist and whitelist on exit
    g_blacklisted_pids.erase(e->pid);
    g_whitelisted_pids.erase(e->pid);
  }

  if (e->event_type == event_type_sched_sched_process_exec)
  {
    // Runs filtering earlier (better perf), but is annoying to debug
    // maybe_update_kernel_blacklist();
  }

  // Check if PID or PPID is blacklisted
  if (g_blacklisted_pids.find(e->pid) != g_blacklisted_pids.end() ||
      g_blacklisted_pids.find(e->ppid) != g_blacklisted_pids.end())
  {
    return true;
  }

  // Accept event
  return false;
}

/* Register the skeleton for later use in filtering */
static void bootstrap_filter__register_skeleton(struct bootstrap_bpf *skel)
{
  if (!skel || !skel->maps.config)
  {
    std::cerr << "Cannot register skeleton: invalid skeleton or map" << std::endl;
    return;
  }

  // Store skeleton in global variable for later use
  g_skel = skel;

  // Initialize the blacklist
  g_blacklisted_pids.clear();
  g_whitelisted_pids.clear();

  // Re-initialize with kernel/init threads and self
  g_blacklisted_pids = {0, 1, 2, static_cast<u32>(getpid())};
  g_whitelisted_pids.clear();
}

#endif // BOOTSTRAP_FILTER_HPP