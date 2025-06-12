# eBPF Tracer

> A high-performance eBPF implementation with hybrid buffering, automatic code generation, and comprehensive event tracking.

## Features

- **Tracepoint capture** comprehensive coverage for all Linux tracepoints, including syscalls
- **2-layer buffering** for large and variable-length data handling (like stdout content)
- **Automatic typegen** for event definitions from TOML configuration
- **Standalone examples** in both C++ and Rust for easy testing and development
- **BPF CO‑RE support** ("Compile Once – Run Everywhere") for kernel portability
- **Rust FFI integration** with shared memory and event streaming

## Architecture

### The Rust-C Interface

eBPF is implemented within a standalone C library (c/ directory) that's linked to Rust (rs/ directory) via a FFI interface with shared memory. This fully decouples our Rust code from eBPF internals.

### Kernel-Userspace Interface

The implementation uses a hybrid 2-layer buffering system:

1. **Ring buffer** contains only metadata and payload flush signals
2. **Per-CPU array maps** stores payload data, with types varying between events

This design overcomes the traditional ring buffer size limitations of eBPF, allowing high-performance capture for large and variable-sized payloads like stdout content.

### Adding new event types

Go to `bootstrap.bpf.c`, and an entry to `TRACEPOINT_LIST`, and a corresponding payload fill function. Like so:

```c
// TRACEPOINT_LIST entry
X(syscalls, sys_enter_openat, trace_event_raw_sys_enter)

// Example payload fill function
static __always_inline void
payload_fill_syscalls_sys_enter_openat(struct trace_event_raw_sys_enter *ctx)
{
  struct payload_kernel_syscalls_sys_enter_openat *p = get_payload_buf_entry();
  if (!p)
    return;

  p->dfd = BPF_CORE_READ(ctx, args[0]);
  p->flags = BPF_CORE_READ(ctx, args[2]);
  p->mode = BPF_CORE_READ(ctx, args[3]);

  void *content_ptr = (void *)BPF_CORE_READ(ctx, args[1]);
  read_into_attr(content_ptr, FILENAME_MAX_SIZE, F_READ_NUL_TERMINATED, &p->filename);
}
```

Then add an entry to `typegen/events.toml` and build (`make` or `cargo build`):

```toml
[syscalls.sys_enter_openat]
id = 1024
comment = "File open, syscall entry"
payload = [
  { name = "dfd", type = "u32" },
  { name = "filename", type = "char[]" },
  { name = "flags", type = "u32" },
  { name = "mode", type = "u32" },
]
```

The typegen system generates:

- C structs and enums (`c/bootstrap.gen.h`)
- Rust types and conversion functions (`rs/types.gen.rs`)
- Consistent event IDs and serialization logic

## Quick Start

### Prerequisites

```sh
# Install build prerequisites (Ubuntu/Debian)
sudo apt install clang libelf1 libelf-dev zlib1g-dev libc6-dev-i386

# Clone with submodules
git clone --recurse-submodules https://github.com/Tracer-Cloud/tracer-client
cd tracer-client/src/ebpf

# Or if already cloned
git submodule update --init --recursive
```

### Building

The build system automatically handles C/C++, eBPF compilation, and typegen:

```sh
# Build C library and example only (fast)
cd ~/tracer-client/ebpf/c && make
```

This produces:

- `c/libbootstrap.a` - Static library for integration
- `c/example` - Standalone C++ example

Or alternatively, you can run:

```sh
# Build both C and Rust libraries and examples (slow)
cd ~/tracer-client/ebpf && cargo build

# Build everything (even slower)
cd ~/tracer-client && cargo build
```

In addition to the above, this also produces:

- `target/debug/libtracer_ebpf` - eBPF library, consumed by Tracer
- `target/debug/example` - Standalone Rust example

### Running Standalone Examples

The two examples behave identically. They log all captured eBPF events as JSON. This is a useful for general development, and also for gathering test data specifically.

**C++ Example:**

```sh
sudo ./c/example
```

**Rust Example:**

```sh
sudo cargo run --bin example
```

Example output:

```json
{"event_id":7515065279460734721,"event_type":"syscalls/sys_enter_write","timestamp_ns":1749737488328669213,"pid":2060594,"ppid":2060565,"upid":2265647914722316300,"uppid":2265616027062207665,"comm":"ls","payload":{"fd":1,"count":69,"content":"Makefile bootstrap-filter.h bootstrap.c bootstrap.templ.h example.cpp\n"}}
{"event_id":7515065279460734722,"event_type":"syscalls/sys_enter_write","timestamp_ns":1749737488328701935,"pid":2060594,"ppid":2060565,"upid":2265647914722316300,"uppid":2265616027062207665,"comm":"ls","payload":{"fd":1,"count":71,"content":"bootstrap-api.h bootstrap.bpf.c bootstrap.gen.h example libbootstrap.a\n"}}
```

## Development

### Debugging

Use `bpf_printk()` for eBPF logs inside `bootstrap.bpf.c`:

```c
bpf_printk("Debug: processing PID %d", pid);
```

View debug output:

```sh
sudo cat /sys/kernel/debug/tracing/trace_pipe
```

### Vendored Dependencies

| **Path**           | **Upstream**                                     |
| ------------------ | ------------------------------------------------ |
| `vendor/libbpf`    | [libbpf](https://github.com/libbpf/libbpf)       |
| `vendor/bpftool`   | [bpftool](https://github.com/libbpf/bpftool)     |
| `vendor/vmlinux.h` | [vmlinux.h](https://github.com/libbpf/vmlinux.h) |

**Updating dependencies:**

```sh
cd vendor/libbpf
git fetch --tags && git checkout v1.5.0
cd ../.. && git add vendor/libbpf && git commit -m "Bump libbpf to v1.5.0"
```

### Code Structure

```
ebpf/
├── c/                    # C/C++ eBPF implementation
│   ├── bootstrap.bpf.c   # Kernel-space eBPF program
│   ├── bootstrap.c       # User-space library
│   ├── bootstrap.gen.h   # Generated types/structs
│   ├── example.cpp       # Standalone C++ example
│   └── Makefile          # Build system
├── rs/                   # Rust FFI bindings
│   ├── binding.rs        # C-Rust interface
│   ├── example.rs        # Standalone Rust example
│   ├── types.gen.rs      # Generated Rust types
│   └── lib.rs            # Library entry point
├── typegen/              # Code generation
│   ├── events.toml       # Event definitions
│   └── typegen.rs        # Code generator
└── build.rs              # Cargo build script
```

## Troubleshooting

### CI/CD Build Failures

**Error:** `fatal error: 'gnu/stubs-32.h' file not found`

This occurs when the CI environment is missing 32-bit development headers on x86_64 systems. The fix depends on your system architecture:

```sh
# Ubuntu/Debian on x86_64
sudo apt-get install -y libc6-dev-i386

# Ubuntu/Debian on ARM64 (aarch64) - 32-bit libraries not needed
sudo apt-get install -y libelf-dev

# Amazon Linux 2 on x86_64
sudo yum install -y glibc-devel.i686

# Amazon Linux 2023 on x86_64 - 32-bit packages no longer available
# If you need 32-bit support, consider using Amazon Linux 2 or containerized AL2
```

**Note:** ARM64 systems don't need 32-bit x86 compatibility libraries. The error typically only occurs on x86_64 systems where clang tries to include both 32-bit and 64-bit headers during eBPF compilation.
