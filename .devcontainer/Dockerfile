FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
RUN rustup toolchain && rustup toolchain install nightly --component rust-src
RUN cargo install bpf-linker && cargo install bindgen-cli && cargo install --git https://github.com/aya-rs/aya -- aya-tool
RUN aya-tool generate task_struct > src/ebpf/kernel/src/gen.rs
