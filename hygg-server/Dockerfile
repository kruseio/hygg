# syntax=docker/dockerfile:1
# Multi-stage build for hygg-server. Build context is the repository root so the
# crate can resolve its workspace path dependencies (e.g. hygg-shared).

FROM rust:1.88-slim AS builder
WORKDIR /build
RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config \
  && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p hygg-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --create-home --uid 10001 hygg
WORKDIR /app
COPY --from=builder /build/target/release/hygg-server /usr/local/bin/hygg-server
RUN mkdir -p /app/data /app/data/blobs /app/data/logs && chown -R hygg:hygg /app
USER hygg
ENV DATABASE_URL=sqlite:///app/data/hygg-server.db \
  HOST=0.0.0.0 \
  PORT=3032 \
  BLOB_DIR=/app/data/blobs \
  LOG_DIR=/app/data/logs \
  RUST_LOG=hygg_server=info,tower_http=info
EXPOSE 3032
CMD ["hygg-server"]
