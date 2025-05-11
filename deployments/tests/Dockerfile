FROM lukemathwalker/cargo-chef:latest-rust-1.86.0 AS chef
WORKDIR /app
RUN apt update && apt install lld clang git -y
FROM chef AS planner
COPY . /app/


# Compute a lock-like file for our project
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
# Up to this point, if our dependency tree stays the same,
# all layers should be cached.

# Add build dependencies
RUN apt update && apt install -y bpftool linux-headers-generic pkg-config
RUN rustup toolchain install nightly-2025-04-15 --component rust-src
RUN cargo install bpf-linker && cargo install bindgen-cli && cargo install --git https://github.com/aya-rs/aya -- aya-tool

COPY . /app/

RUN cd /app/src/ebpf-libbpf && git submodule update --init --recursive && cd /app/

RUN aya-tool generate task_struct > /app/src/ebpf/kernel/src/gen.rs

# Build our project
RUN cargo build --release 


FROM debian:bookworm-slim AS runtime

WORKDIR /app


# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    openssl ca-certificates \
    libsasl2-modules-gssapi-mit libsasl2-dev \
    python3 python3-pip python3-venv \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*


COPY --from=builder /app/tracer.toml /root/.config/tracer/tracer.toml
COPY --from=builder /app/target/release/tracer_cli /usr/local/bin/tracer

# copy test files
COPY --from=builder /app/test-files/scripts /app/test-files/scripts
COPY --from=builder /app/test-files/test-data-samples /app/test-files/test-data-samples

ENTRYPOINT ["sh", "-c"]


CMD ["tracer"]
