# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.86-slim-bookworm AS builder

# Install protobuf compiler (required by tonic_build in axon-daemon/build.rs)
RUN apt-get update && apt-get install -y --no-install-recommends \
      protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy full source tree
COPY . .

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
