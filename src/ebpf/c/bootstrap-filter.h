#ifndef BOOTSTRAP_FILTER_H
#define BOOTSTRAP_FILTER_H

#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <ctype.h>
#include <strings.h>
#include <unistd.h>
#include <stdlib.h>
#include <errno.h>

#include <bpf/libbpf.h>
#include "bootstrap.gen.h"
#include "bootstrap.skel.h"

typedef unsigned long long u64;
typedef unsigned int u32;

/* ========================================================================== */
/* PID SET DATA STRUCTURES AND OPERATIONS                                    */
/* ========================================================================== */
/* Simple array-based PID sets for tracking blacklisted/whitelisted processes */

/* hard upper-bound – this many tracked concurrent processes is plenty for most workloads */
#define PIDSET_CAP 8192

/* Trade-off: kernel filtering improves performance but makes debugging harder */
static const bool ENABLE_KERNEL_BLACKLIST_SYNC = false;

struct pid_set
{
    u32 data[PIDSET_CAP] __attribute__((aligned(64))); // memory-aligned for perf
    size_t count;
} __attribute__((aligned(64)));

static inline bool pidset_has(const struct pid_set *s, u32 v)
{
    for (size_t i = 0; i < s->count; ++i)
        if (s->data[i] == v)
            return true;
    return false;
}

static inline void pidset_add(struct pid_set *s, u32 v)
{
    if (s->count >= PIDSET_CAP || pidset_has(s, v))
        return;
    s->data[s->count++] = v;
}

static inline void pidset_del(struct pid_set *s, u32 v)
{
    for (size_t i = 0; i < s->count; ++i)
        if (s->data[i] == v)
        {
            s->data[i] = s->data[--s->count];
            return;
        }
}

/* ========================================================================== */
/* GLOBAL STATE AND UTILITY FUNCTIONS                                        */
/* ========================================================================== */

static struct bootstrap_bpf *g_skel = NULL;
static struct pid_set g_blacklisted_pids = {{0}, 0};
static struct pid_set g_whitelisted_pids = {{0}, 0};
static u32 g_kernel_subset[MAX_BLACKLIST_ENTRIES] = {0};

static int cmp_u32(const void *a, const void *b)
{
    u32 x = *(const u32 *)a, y = *(const u32 *)b;
    return (x > y) - (x < y);
}

/* naïve case-insensitive "needle in haystack" test */
static inline bool s_icontains(const char *hay, const char *needle)
{
#ifdef __GLIBC__
    return strcasestr(hay, needle) != NULL;
#else
    /* fallback: copy and convert to lower-case first */
    char buf[256];
    size_t n = strlen(hay);
    if (n >= sizeof(buf))
        n = sizeof(buf) - 1;
    for (size_t i = 0; i < n; ++i)
        buf[i] = (char)tolower((unsigned char)hay[i]);
    buf[n] = '\0';
    return strstr(buf, needle) != NULL;
#endif
}

/* read cmdline for given PID, convert null bytes to spaces */
static bool get_cmdline(u32 pid, char *buffer, size_t buffer_size)
{
    char path[64];
    snprintf(path, sizeof(path), "/proc/%u/cmdline", pid);

    FILE *f = fopen(path, "r");
    if (!f)
        return false;

    size_t n = fread(buffer, 1, buffer_size - 1, f);
    fclose(f);

    if (n == 0)
        return false;

    buffer[n] = '\0';
    for (size_t i = 0; i < n; ++i)
        if (buffer[i] == '\0')
            buffer[i] = ' ';

    return true;
}

/* ========================================================================== */
/* PROCESS FILTERING LOGIC - MAIN CUSTOMIZATION POINT                        */
/* ========================================================================== */

/*
 * ⚠️  IMPORTANT: PRIMARY CUSTOMIZATION FUNCTION ⚠️
 *
 * This function determines which processes should be blacklisted (ignored)
 * by the tracer. It's the most likely function to need modification when
 * adapting to new product requirements or deployment environments.
 *
 * NOTE: This is a low-level, performance-critical filtering pass. For more
 * sophisticated filtering logic, consider implementing it outside the eBPF
 * module where you have access to richer APIs and debugging tools.
 *
 * MODIFY THIS FUNCTION to add new process patterns to ignore:
 * - Development tools (editors, build systems, version control)
 * - System utilities (monitoring, maintenance scripts)
 * - Infrastructure processes specific to your environment
 *
 * Returns: true if process should be blacklisted (ignored), false otherwise
 */
static bool should_blacklist_process(const struct event_header_kernel *e)
{
    static const char *pats[] = {"vscode", "example", "tracer", "sleep", "irqbalance", "git", "sshd", "ps"};
    const size_t NPAT = sizeof(pats) / sizeof(pats[0]);

    for (size_t i = 0; i < NPAT; ++i)
        if (s_icontains(e->comm, pats[i]))
            return true;

    char line[4096];
    if (!get_cmdline(e->pid, line, sizeof(line)))
        return false;

    for (size_t i = 0; i < NPAT; ++i)
        if (s_icontains(line, pats[i]))
            return true;

    // Skip non-interactive processes launched by cursor
    if (s_icontains(line, "cursor") && !s_icontains(line, "terminal"))
        return true;

    return false;
}

/* ========================================================================== */
/* KERNEL SYNCHRONIZATION                                                     */
/* ========================================================================== */

/* push the first ≤ MAX_BLACKLIST_ENTRIES PIDs to the kernel map (ascending) */
static void maybe_update_kernel_blacklist(void)
{
    if (!g_skel || !g_skel->maps.config)
        return;

    u32 sorted[PIDSET_CAP];
    memcpy(sorted, g_blacklisted_pids.data,
           g_blacklisted_pids.count * sizeof(u32));
    qsort(sorted, g_blacklisted_pids.count, sizeof(u32), cmp_u32);

    size_t n = g_blacklisted_pids.count;
    if (n > MAX_BLACKLIST_ENTRIES)
        n = MAX_BLACKLIST_ENTRIES;

    if (memcmp(sorted, g_kernel_subset, n * sizeof(u32)) == 0)
        return; /* no change */

    memcpy(g_kernel_subset, sorted, n * sizeof(u32));
    memset(g_kernel_subset + n, 0,
           (MAX_BLACKLIST_ENTRIES - n) * sizeof(u32));

    for (size_t i = 0; i < MAX_BLACKLIST_ENTRIES; ++i)
    {
        u32 key = CONFIG_PID_BLACKLIST_0 + (u32)i;
        u64 val = g_kernel_subset[i];
        bpf_map__update_elem(g_skel->maps.config,
                             &key, sizeof(key),
                             &val, sizeof(val),
                             BPF_ANY);
    }
}

/* ========================================================================== */
/* PUBLIC API                                                                 */
/* ========================================================================== */

/* public: called from bootstrap.c before first event recieved */
static void bootstrap_filter__register_skeleton(struct bootstrap_bpf *skel)
{
    g_skel = skel;

    g_blacklisted_pids.count = g_whitelisted_pids.count = 0;
    pidset_add(&g_blacklisted_pids, 0);
    pidset_add(&g_blacklisted_pids, 1);
    pidset_add(&g_blacklisted_pids, 2);
    pidset_add(&g_blacklisted_pids, (u32)getpid());
}

/* public: called from bootstrap.c on every event */
static bool bootstrap_filter__should_skip(struct event_header_kernel *e)
{
    // Invalidate old list entries on PID reuse
    if (e->event_type == event_type_sched_sched_process_exec)
    {
        pidset_del(&g_blacklisted_pids, e->pid);
        pidset_del(&g_whitelisted_pids, e->pid);
    }

    // Add new PID to lists
    if (!pidset_has(&g_blacklisted_pids, e->pid) &&
        !pidset_has(&g_whitelisted_pids, e->pid))
    {
        if (should_blacklist_process(e))
        {
            pidset_add(&g_blacklisted_pids, e->pid);
        }
        else
        {
            pidset_add(&g_whitelisted_pids, e->pid);
        }
    }

    bool should_skip = pidset_has(&g_blacklisted_pids, e->pid) || pidset_has(&g_blacklisted_pids, e->ppid);

    // Tidy up old entries
    if (e->event_type == event_type_sched_sched_process_exit)
    {
        pidset_del(&g_blacklisted_pids, e->pid);
        pidset_del(&g_whitelisted_pids, e->pid);
    }

    /* Update kernel-side blacklist for improved filtering performance */
    if (ENABLE_KERNEL_BLACKLIST_SYNC && e->event_type == event_type_sched_sched_process_exec)
        maybe_update_kernel_blacklist();

    return should_skip;
}

#endif /* BOOTSTRAP_FILTER_H */
