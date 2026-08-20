# syntax=docker/dockerfile:1
# aiapp-mb Web service multi-stage build: Rust compilation + MoonBit toolchain + front-end static assets

# ---------- Build stage ----------
FROM rust:1.83-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        curl ca-certificates pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Install the MoonBit toolchain (provides the moon command, used to compile WASM on demand at runtime)
RUN curl -fsSL https://cli.moonbitlang.com/install/unix.sh | bash
ENV PATH="/root/.moon/bin:${PATH}"

# Pre-warm the wasm-gc toolchain (best effort; failures are fine, the first runtime build downloads it automatically)
RUN mkdir -p /tmp/warm && cd /tmp/warm && \
    ( /root/.moon/bin/moon new --name warmapp >/dev/null 2>&1 && \
      cd warmapp && /root/.moon/bin/moon build --target wasm-gc >/dev/null 2>&1 ) || true

WORKDIR /build
COPY . .
RUN cargo build --release -p aiapp-web

# ---------- Runtime stage ----------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the MoonBit toolchain (CLI + pre-warmed toolchain cache)
COPY --from=builder /root/.moon /root/.moon
ENV PATH="/root/.moon/bin:${PATH}"
# Runtime toolchain download directory (mounted to a persistent volume to avoid re-downloading)
ENV MOON_HOME=/data/moon

WORKDIR /app
COPY --from=builder /build/target/release/aiapp-web /app/bin/aiapp-web
COPY --from=builder /build/crates/aiapp-web/src/templates /app/templates
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Runtime configuration defaults (overridable via environment variables)
ENV HOST=0.0.0.0
ENV PORT=8080
ENV STATIC_DIR=/app/templates
ENV STORAGE_BACKEND=local
ENV STORAGE_LOCAL_ROOT=/data/storage
ENV AIAPP_BACKEND=mock

EXPOSE 8080
VOLUME ["/data"]
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["/app/bin/aiapp-web"]
