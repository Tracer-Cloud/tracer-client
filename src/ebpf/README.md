# ebpf

NOTE: this README is badly outdated.

> A minimal BPF implementation based on [`libbpf/libbpf-bootstrap`](https://github.com/libbpf/libbpf-bootstrap) (examples/c/bootstrap).

## Features

- Tracks process starts and exits via tracepoints
- BPF CO‑RE (“Compile Once – Run Everywhere”) for kernel portability
- Demonstrates core eBPF concepts
  - Tracepoints, maps, ring buffers, and configurable globals
- Rust binding with shared memory, for integration into Tracer

## Vendored dependencies (git submodules)

| **Path**            | **Upstream**                                      |
|---------------------|---------------------------------------------------|
| `vendor/libbpf`     | [libbpf](https://github.com/libbpf/libbpf)        |
| `vendor/bpftool`    | [bpftool](https://github.com/libbpf/bpftool)      |
| `vendor/vmlinux.h`  | [vmlinux.h](https://github.com/libbpf/vmlinux.h)  |

A **git submodule** is simply a pointer (a *gitlink*) to a specific commit in another repository.
We put these gitlinks in the `vendor/` folder.
Advantages over linking against system libraries include:

* deterministic builds—everyone compiles against the same commits;
* zero external build‑time dependencies (no need for system libbpf or bpftool packages);
* easy upgrades: bump the submodule, commit, push.

## Quick start (development)

```sh
# Install build prerequisites (Ubuntu/Debian)
sudo apt install clang libelf1 libelf-dev zlib1g-dev

# Clone *and* pull submodules in one go
git clone --recurse-submodules https://github.com/Tracer-Cloud/tracer-client
cd src/ebpf

# (Or, if you already cloned)
git submodule update --init --recursive
```

**Building**:

> Note: `cargo build` is configured to run `make` behind-the-scenes. This is only for people actively working on eBPF, rather than merely consuming the crate.
>
> The first build will be slow, but subsequent builds will be fast, thanks to partial caching.

```sh
cd c
make
```

This produces:

- `./example`: standalone binary, which just logs captured events when executed (useful for debugging).
- `./libbootstrap.a`: linkable object used as input for Tracer binary compilation.

Run the example with:

```sh
sudo ./example
```

The output should look something like:

```sh
{"event_type":"process_exec","timestamp":"1970-01-07 06:48:44.807862128","pid":1258136,"ppid":1244910,"comm":"git","argc":9,"argv":["/usr/bin/git","-c","core.quotepath=false","-c","color.ui=false","rev-parse","--verify","--end-of-options","1252231^{commit}"]}
{"event_type":"process_exit","timestamp":"1970-01-07 06:48:44.809130125","pid":1258136,"ppid":1244910}
{"event_type":"process_exec","timestamp":"1970-01-07 06:48:45.386948556","pid":1258137,"ppid":1206440,"comm":"sh","argc":3,"argv":["/bin/sh","-c","which ps"]}
{"event_type":"process_exec","timestamp":"1970-01-07 06:48:45.387484766","pid":1258138,"ppid":1258137,"comm":"which","argc":3,"argv":["/bin/sh","/usr/bin/which","ps"]}
{"event_type":"process_exit","timestamp":"1970-01-07 06:48:45.387959373","pid":1258138,"ppid":1258137}
{"event_type":"process_exit","timestamp":"1970-01-07 06:48:45.388083524","pid":1258137,"ppid":1206440}
```

**Updating vendored dependencies**:

```sh
# Move to the submodule you want to update
cd vendor/libbpf
git fetch --tags
git checkout v1.5.0      # or another tag/branch/commit

# Return to the main repo, and commit change
cd ../..
git add vendor/libbpf
git commit -m "Bump libbpf to v1.5.0"
```

**Debugging**:

Use `bpf_printk()` for logging from `.bpf.c` files:

```c
bpf_printk("Test %s", my_value);
```

We haven't added log-forwarding yet, so to see these logs go to another terminal and run:

```sh
sudo cat /sys/kernel/debug/tracing/trace_pipe
```

## Software design

**The Rust-C interface**

eBPF is implemented within a standalone C library (`c/` directory) that's linked to Rust (`rs/` directory) via a FFI interface with shared memory. This fully decouples our Rust code from eBPF internals.

Here, `binding.rs` allocates a buffer the C library can write to asynchronously. The library does so and notifies `binding.rs` of writes via a callback. The callback then sends the events onwards and allocates a new buffer, completing the cycle.

**The kernel-userspace interface**

TODO

```txt
/*
 * eBPF Tracer with Hybrid Buffering System
 *
 * This implementation uses a two-level buffering system:
 * 1. Per-CPU array maps store the actual data in 4KB page-sized entries
 * 2. Ringbuf contains only metadata (indices/offsets to the data in the per-CPU arrays)
 *
 * Key components:
 * - data_buffer: Per-CPU array that stores actual event data in 4KB pages
 * - buffer_states: Per-CPU state tracking (current page, offset, etc.)
 * - rb: Ringbuf for metadata only (event type, timestamp, process info, buffer location)
 *
 * Usage flow:
 * 1. buf_reserve(): Allocate buffer space for data chunks
 * 2. Write data directly to the reserved buffer space
 * 3. submit_event(): Submit metadata to ringbuf with references to the data
 *
 * This approach efficiently handles variable-length data like stdout capture
 * without the size limitations of using ringbuf directly.
 */
```

## Future development

To explore in the future:

1. **Codegen**: Adding collection of new tracepoints should be as simple as adding an entry to the codegen config with the name of the tracepoint and attributes of interest.
    - For example: 
      ```json
      {
        "tracepoint": "syscall:sys_enter_openat",
        "attributes": ["filename", "flags", "mode"]
      }
      ```
    - The Rust types and C code would be generated by a build script.
2. **Better working format**: Performance can be improved by 100x with some behaviour / interface adjustments.
    - **Reduce memory churn**: Instead of allocating new memory for each incoming event, we should reuse buffers for previously-processed events.
    - **Separate buffers by event type**: Vectors of tagged unions don't align with the performance potential of modern hardware. Routing events into separate buffers should happen early, probably in `bootstrap.c`.
    - **Batch processing**: Processing in batches of 16/64/256/etc events would enable much faster transforms. These batches should have an SoA layout (ie, column-oriented), in alignment with the internal format of dataframe libraries like Polars.
3. **Testing across multiple environments**: In theory, we _should_ have very good support across Linux versions with CO-RE. It would be great to validate that in practice. Testing on multiple OS / kernel versions would confirm correctness and help identify compatiability issues if any exist.


## Programming Guide

No vectors, strings.

No true functions, loops, pointers, error handling or libraries (including standard libraries).