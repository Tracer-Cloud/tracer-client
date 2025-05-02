# Use Ubuntu 22.04 as base image
FROM ubuntu:22.04

# Set environment variables
ENV DEBIAN_FRONTEND=noninteractive
ENV RUST_LOG=debug
ENV PATH="/root/.cargo/bin:${PATH}"
ENV AWS_DEFAULT_REGION=us-east-1

# Install system dependencies based on Cargo.toml requirements
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    git \
    libssl-dev \
    pkg-config \
    python3 \
    python3-pip \
    unzip \
    wget \
    openjdk-17-jdk \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN rustup default stable
RUN rustup toolchain install nightly --component rust-src
RUN cargo install bpf-linker && cargo install bindgen-cli && cargo install --git https://github.com/aya-rs/aya -- aya-tool

# Create directories for Tracer
RUN mkdir -p /opt/tracer /etc/tracer

# Copy the entire project
COPY . /opt/tracer/src
RUN aya-tool generate task_struct > /opt/tracer/src/ebpf/kernel/src/gen.rs

WORKDIR /opt/tracer/src

# Build Tracer with release profile
RUN cargo build --release

# Create symbolic link and set permissions
RUN chmod +x /opt/tracer/src/target/release/tracer_cli && \
    ln -s /opt/tracer/src/target/release/tracer_cli /usr/local/bin/tracer

# Add version information
LABEL version="0.0.130"
LABEL org.opencontainers.image.source="https://github.com/tracer-cloud/tracer-cloud"

# Default command
ENTRYPOINT ["tracer"]
CMD ["--help"]
