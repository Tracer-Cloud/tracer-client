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
git clone --recurse-submodules https://github.com/
cd ebpf-libbpf

# (Or, if you already cloned)
git submodule update --init --recursive
```

**Building**:

> The first build will be slow, but subsequent builds will be fast, thanks to caching.

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
