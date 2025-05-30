# ebpf-libbpf

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
cd src/ebpf-libbpf

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
- `./bootstrap.a`: linkable object used as input for Tracer binary compilation.

Run the example with:

```sh
sudo ./example
```

The output should look something like:

```sh
TIME     EVENT COMM      PID     PPID    FILENAME/EXIT CODE
00:21:22 EXIT  python3.8 4032353 4032352 [0] (123ms)
00:21:22 EXEC  mkdir     4032379 4032337 /usr/bin/mkdir
00:21:22 EXIT  mkdir     4032379 4032337 [0] (1ms)
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

## Software design

**The Rust-C interface**

eBPF is implemented within a standalone C library (`c/` directory) that's linked to Rust via a FFI interface with shared memory. This fully decouples our Rust code from eBPF internals.

Here, `binding.rs` allocates a buffer the C library can write to asynchronously. The library does so and notifies `binding.rs` of writes via a callback. The callback then sends the events onwards and allocates a new buffer, completing the cycle.

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
