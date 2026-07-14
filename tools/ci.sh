#!/usr/bin/env bash

# Deps
# cargo install --locked cargo-audit cargo-edit cargo-udeps cargo-geiger cargo-crev cargo-deny

set -Eeuo pipefail

# Every leg below assumes the workspace root as cwd (cargo member selection, the
# fork-manifest checkout, the relative hygg-pwa paths), so anchor there rather
# than trusting the caller's directory.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# hygg-cff-parser and hygg-pdf-extract are verbatim forks of upstream crates,
# kept unmodified so they can be re-synced from upstream with a one-line
# manifest change. They still build (and are type-checked) as dependencies of
# cli-pdf-to-text, but are excluded from the mutating / linting tooling below:
#   - --exclude works for the cargo built-ins (check/fix/clippy/test) + udeps
#   - cargo fmt has no --exclude, so it skips them via rustfmt.toml's `ignore`
#   - cargo upgrade has no per-member exclude (its --exclude filters by
#     dependency name), so the fork manifests are restored right after it runs
FORKS=(--exclude hygg-cff-parser --exclude hygg-pdf-extract)

# hygg-pwa's real artifact is a wasm32-unknown-unknown bundle: the Leptos/web-sys
# stack is cfg(target_arch = "wasm32")-gated, so its *host* build is only a thin
# `trunk` launcher (`cargo run -p hygg-pwa`). Linting that launcher adds nothing,
# so it is excluded from every host-target workspace command below and covered
# instead by a dedicated wasm step (`ci_wasm`). cargo fmt is target-independent,
# so --all still formats it.
# hygg-tauri pulls the heavy Tauri stack and embeds the hygg-pwa `dist/` bundle
# (its `generate_context!` needs it built first), so it's excluded from the
# generic host `--workspace` legs below and covered by its own `ci_tauri` leg
# (which builds the bundle first) — same pattern as hygg-pwa / `ci_wasm`.
HOST_ONLY=("${FORKS[@]}" --exclude hygg-pwa --exclude hygg-tauri)

ci () {
  cargo update --verbose
  cargo upgrade --verbose
  git checkout -- hygg-cff-parser/Cargo.toml hygg-pdf-extract/Cargo.toml
  cargo audit

  cargo +nightly check --workspace "${HOST_ONLY[@]}"
  cargo +nightly fix --allow-dirty --workspace "${HOST_ONLY[@]}"
  cargo +nightly clippy --workspace "${HOST_ONLY[@]}" --all-targets --all-features -- -D warnings
  cargo +nightly fmt --all

  # Source hygiene: no authored .rs file may exceed the LOC budget. Run after
  # fmt so the counts reflect canonical formatting.
  tools/loc-gate.sh

  cargo +nightly test --workspace "${HOST_ONLY[@]}"

  # TTS narration is feature-gated, so the default test run compiles its
  # phonemize/alignment regression tests out. Run them explicitly to guard the
  # espeak punctuation -> Kokoro pause-token contract across dep bumps (the
  # real-espeak test self-locates the build-vendored espeak-ng-data).
  cargo +nightly test -p cli-text-reader --features tts --lib

  cargo +nightly udeps --workspace "${HOST_ONLY[@]}" --all-targets

  ci_wasm
  ci_tauri
}

# Native shell leg for hygg-tauri (the Tauri v2 desktop/mobile app that wraps the
# hygg-pwa UI). It's a workspace member so the host --workspace legs above
# already check/clippy/test it, but its real artifact needs the frontend bundle
# in place (generate_context! embeds ../hygg-pwa/dist). This builds the app
# binary against a fresh Trunk bundle so a broken IPC command or config is caught
# here, without requiring the mobile SDKs (those are CI-runner specific). Store
# bundles / mobile builds are produced by `cargo tauri build` / `android|ios
# build` on the release runners. It reuses the wasm leg's Trunk output.
ci_tauri () {
  ( cd hygg-pwa && trunk build --release )
  cargo +nightly clippy -p hygg-tauri --all-targets -- -D warnings
  cargo +nightly build -p hygg-tauri
  # Native extraction commands (base64 decode + the txt/pdf/epub pipeline).
  cargo +nightly test -p hygg-tauri
}

# Browser/wasm leg for hygg-pwa. Compiles + lints the PWA for wasm32, confirms
# the Trunk bundle assembles, and guards the `cargo install hygg` isolation
# invariant (the CLI's native dependency tree must never pull the PWA's
# Leptos/wasm stack).
ci_wasm () {
  rustup +nightly target add wasm32-unknown-unknown >/dev/null 2>&1 || true

  cargo +nightly clippy -p hygg-pwa --target wasm32-unknown-unknown --all-features -- -D warnings
  cargo +nightly build -p hygg-pwa --target wasm32-unknown-unknown
  cargo +nightly udeps -p hygg-pwa --target wasm32-unknown-unknown
  ( cd hygg-pwa && trunk build --release )

  # (hygg-gui is a native desktop app — no wasm leg here; its build is covered by
  # the host --workspace legs above. The browser reader is hygg-pwa.)

  # Isolation guard: fail if any Leptos/wasm or iced/GUI crate leaks into the
  # published CLI's normal dependency tree (cargo install hygg must never pull
  # the PWA's Leptos/wasm stack or hygg-gui's iced/wgpu stack).
  if cargo tree -p hygg -e normal --prefix none 2>/dev/null \
       | grep -Eiq '^(leptos|gloo|wasm-bindgen|web-sys|js-sys|iced|wgpu|winit|tauri)'; then
    echo "ERROR: hygg dependency tree leaked PWA/GUI/Tauri crates (cargo install hygg must stay clean)" >&2
    exit 1
  fi
}

ci
