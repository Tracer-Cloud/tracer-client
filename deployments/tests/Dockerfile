FROM lukemathwalker/cargo-chef:latest-rust-1.81.0 AS chef
WORKDIR /app
RUN apt update && apt install lld clang -y
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
COPY . /app/

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
COPY --from=builder /app/target/release/tracer_cli /usr/local/bin/

# copy test files
COPY --from=builder /app/test-files/scripts /app/test-files/scripts
COPY --from=builder /app/test-files/test-data-samples /app/test-files/test-data-samples

ENTRYPOINT ["sh", "-c"]


CMD ["tracer"]
