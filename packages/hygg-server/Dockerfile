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
# OCI image metadata, for anything that builds this image outside CI (docker
# compose, plain `docker build`); the GHCR workflow sets the same set through
# docker/metadata-action. `licenses` is Elastic-2.0 because that is what
# hygg-server ships under (see packages/hygg-server/LICENSE) — the correct value
# for `docker inspect`, license scanners and registries that read it.
#
# It does NOT change the license shown on the GHCR *package page*, though:
# GitHub sources that badge from the linked repository's detected license — the
# reader's AGPL-3.0 at the repo root — with no per-package override. Of these
# labels only `description` reaches the GHCR page (as the short summary); GHCR
# renders no README for a container package, so `documentation` is a link only.
LABEL org.opencontainers.image.title="hygg-server" \
  org.opencontainers.image.description="Self-hostable, multi-tenant sync server for the hygg reader. Sync documents, reading progress, bookmarks, highlights and notes across devices. Single Rust binary backed by SQLite." \
  org.opencontainers.image.licenses="Elastic-2.0" \
  org.opencontainers.image.source="https://github.com/kruseio/hygg" \
  org.opencontainers.image.documentation="https://github.com/kruseio/hygg/blob/main/packages/hygg-server/README.md"
# pandoc is a runtime dependency, not a nicety: POST /api/v1/convert shells out
# to it for DOCX/ODT/RTF (src/api/convert.rs), so without it the container
# answers 500 for every format the endpoint advertises. PDF/EPUB/text are pure
# Rust; the OCR engine is too, but its ONNX models are no longer embedded — they
# are downloaded from this project's `ocr-models-v1.0` release over HTTPS on the
# first scanned-PDF convert (hence ca-certificates) and cached under
# HYGG_OCR_MODEL_DIR, which points at the data volume below so the ~10 MB fetch
# happens once for the life of the volume, not once per container start.
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
# The data directory is created empty, and stays that way until the server
# claims it: everything under it (the database, hygg-logs/, models/) is made on
# demand at runtime. Seeding those subdirectories here would be actively wrong —
# Docker copies image content into a *named* volume when it is first mounted, so
# the server would meet a non-empty, unmarked directory on a first boot and
# refuse it as somebody else's (see src/data_dir.rs).
RUN mkdir -p /app/data && chown -R hygg:hygg /app
USER hygg
# HYGG_DATA_DIR stays /app/data — the container side of the mount is this
# image's interface, and renaming it would strand every existing
# `-v <host>:/app/data` deployment on an upgrade: their data would sit
# unmounted while the server started fresh in the container's writable layer.
# The host side is where a name can collide, and the docs there say hygg-data.
ENV HYGG_DATA_DIR=/app/data \
  DATABASE_URL=sqlite:///app/data/hygg-server.db \
  HOST=0.0.0.0 \
  PORT=3032 \
  LOG_DIR=/app/data/hygg-logs \
  HYGG_OCR_MODEL_DIR=/app/data/models/ocr \
  RUST_LOG=hygg_server=info,tower_http=info
EXPOSE 3032
CMD ["hygg-server"]
