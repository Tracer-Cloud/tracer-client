FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

RUN apt-get update

RUN apt-get install -y \
    build-essential \
    curl

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup toolchain install stable && rustup toolchain install nightly-2025-04-15 --component rust-src

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall cargo-nextest --secure

RUN cargo install bpf-linker && cargo install bindgen-cli && cargo install --git https://github.com/aya-rs/aya -- aya-tool
