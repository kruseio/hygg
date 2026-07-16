# syntax=docker/dockerfile:1
# Multi-stage build for hygg-server. Build context is the repository root so the
# crate can resolve its workspace path dependencies (e.g. hygg-shared).
#
# The dependency build is split out with cargo-chef so it lands in its own layer,
# keyed on the manifests alone. Without that, the `COPY . .` below invalidates on
# any source change anywhere in the workspace and every build recompiles the
# whole tree (axum, sea-orm, webauthn, the bundled tract OCR engine) from cold —
# which the CI image build does on every pull request.

FROM rust:1.88-slim AS chef
WORKDIR /build
# libssl-dev is not optional, despite sea-orm being configured for rustls:
# webauthn-rs -> webauthn-rs-core -> webauthn-attestation-ca pulls the `openssl`
# crate, whose build script needs the system headers. Without it the build dies
# in openssl-sys's build script.
RUN apt-get update \
  && apt-get install -y --no-install-recommends pkg-config libssl-dev \
  && rm -rf /var/lib/apt/lists/* \
  && cargo install cargo-chef --locked

# Reduce the workspace to a dependency recipe: manifests only, no source.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# Only the recipe is copied first, so this layer is reused until a Cargo.toml or
# Cargo.lock actually changes.
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release -p hygg-server --recipe-path recipe.json
COPY . .
RUN cargo build --release -p hygg-server

FROM debian:bookworm-slim AS runtime
# OCI labels for anything that builds this image outside CI (docker compose,
# plain `docker build`). The GHCR workflow applies the same set through
# docker/metadata-action — with an explicit `licenses` override there, because
# the action's default derives the license from the *repository* (AGPL-3.0,
# the reader's license) while hygg-server ships under Elastic-2.0 (see
# packages/hygg-server/LICENSE); the auto label is what made the GHCR package
# page advertise AGPL. `description` is what GHCR shows on the package page.
LABEL org.opencontainers.image.title="hygg-server" \
  org.opencontainers.image.description="Self-hostable, multi-tenant sync server for the hygg reader. Sync documents, reading progress, bookmarks, highlights and notes across devices. Single Rust binary backed by SQLite." \
  org.opencontainers.image.licenses="Elastic-2.0" \
  org.opencontainers.image.source="https://github.com/kruseio/hygg" \
  org.opencontainers.image.documentation="https://github.com/kruseio/hygg/blob/main/packages/hygg-server/README.md"
# pandoc is a runtime dependency, not a nicety: POST /api/v1/convert shells out
# to it for DOCX/ODT/RTF (src/api/convert.rs), so without it the container
# answers 500 for every format the endpoint advertises. PDF/EPUB/text and the
# bundled OCR are pure Rust and need nothing here.
#
# libssl3 is named explicitly even though ca-certificates happens to pull it in:
# the binary links it (see the builder stage), so it should not depend on a
# neighbour's transitive dependency to keep working.
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates libssl3 pandoc \
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
