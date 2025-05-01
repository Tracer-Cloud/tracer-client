# Documentation To Run eBPF Specific Functionality 
How to run eBPF:
- Linux Ubuntu v22
- Run the eBPF binary on Root(!) 
- sudo su - root

```bash 
# Installation May 1st 
sudo curl -sSL https://blaginin-eng-26-fix-ebpf-eve.tracer-client.pages.dev/installation-script-development.sh | bash && source ~/.bashrc 
```

```bash
# Run the pipeline as sudo 
sudo tracer init --pipeline-name ebpf_test --environment sandbox_test --user-operator vincent --pipeline-type rnaseq
```






## Running the eBPF build

```bash
RUST_LOG=info cargo run --config 'target."cfg(all())".runner="sudo -E"' -- init --pipeline-name testing --run-id parallel-tag --environment integrations --user-operator inte --pipeline-type generic --no-daemonize
```

## Required Installation for eBPF with Aya
### Check if bpf-linker is installed
```bash
cargo install bpf-linker
```

```bash
which bpf-linker
```

### You need rust up component for the nighthly toolchain
```bash
rustup component add rust-src --toolchain nightly-aarch64-unknown-linux-gnu
rustup toolchain install nightly
rustup default nightly
```

### Linux Headers
Needed for BPF program compilation:

```bash
sudo apt-get install -y linux-headers-$(uname -r)
```

### BPF Tools and Libraries

```bash
sudo apt-get install -y libbpf-dev bpftool
```

### Build Essentials and Dependencies

```bash
sudo apt-get install -y build-essential pkg-config libelf-dev
```

### BPF Linker Installation

Install the bpf-linker tool used by Aya:

```bash
cargo install bpf-linker
```

### Additional Requirements

For complete eBPF functionality, you may also need to install:

```bash
sudo apt-get update
sudo apt-get install -y llvm clang
```

Note: Ensure you're running a kernel with good BTF support (5.7+) for full eBPF capabilities.

Kernel version check:
- uname -r

BTF File check:
- ls -la /sys/kernel/btf/vmlinux


### You need Aya tool to 
at this point everything should be working, but because I generated headers for kernel 5.15 , it will crash under ubuntu with 6.8 kernel. I'm happy to update the generated file, but want to explain it to you as well: (edited) 


```bash
cargo install bindgen-cli
cargo install --git https://github.com/aya-rs/aya -- aya-tool
```

### Generate new bindings with Aya
```bash
aya-tool generate task_struct > src/ebpf/kernel/src/gen.rs
```


# Tracer test 
cargo run init --pipeline-name airflow_test --environment sandbox_test --user-operator vincent --pipeline-type rnaseq
