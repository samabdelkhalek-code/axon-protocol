# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.86-slim-bookworm AS builder

WORKDIR /build

# Cache dependency compilation separately from source
COPY Cargo.toml Cargo.lock ./
COPY axon-core/Cargo.toml   axon-core/Cargo.toml
COPY axon-daemon/Cargo.toml axon-daemon/Cargo.toml
COPY axon-sdk/Cargo.toml    axon-sdk/Cargo.toml
COPY axon-cli/Cargo.toml    axon-cli/Cargo.toml

# Create stub source files so cargo can resolve the dependency graph
RUN mkdir -p axon-core/src axon-core/benches axon-daemon/src axon-sdk/src axon-cli/src && \
    echo "pub fn stub() {}" | tee \
      axon-core/src/lib.rs axon-sdk/src/lib.rs > /dev/null && \
    echo "fn main() {}" | tee \
      axon-daemon/src/main.rs axon-cli/src/main.rs > /dev/null && \
    echo "fn main() {}" > axon-core/benches/similarity.rs && \
    cargo build --release -p axon-daemon && \
    rm -rf axon-core/src axon-core/benches axon-daemon/src axon-sdk/src axon-cli/src

# Copy and build the real source
COPY axon-core/src   axon-core/src
COPY axon-daemon/src axon-daemon/src
COPY axon-sdk/src    axon-sdk/src
COPY axon-cli/src    axon-cli/src

ENV RUSTFLAGS="-C target-cpu=native"
RUN cargo build --release -p axon-daemon

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/axond /usr/local/bin/axond

# Non-root user for security
RUN useradd -m -u 1000 axon
USER axon

WORKDIR /home/axon

# Data directory for key + sled database
RUN mkdir -p .axon

EXPOSE 7777

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD axond --help > /dev/null 2>&1 || exit 1

ENV AXON_LISTEN_ADDR=/ip4/0.0.0.0/tcp/7777
ENV AXON_LOG_LEVEL=info

ENTRYPOINT ["axond"]
