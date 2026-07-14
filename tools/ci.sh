#!/usr/bin/env bash

# The workspace check suite, in two modes:
#
#   ./tools/ci.sh          The maintainer's local pass. Refreshes dependencies,
#                          applies every fix that can be applied, then runs every
#                          gate below over the result.
#
#   ./tools/ci.sh <leg>    One gate, alone. This is how .github/workflows/ci.yml
#                          runs — a job per leg — so a pull request into main is
#                          held to *this* file rather than to a second copy of
#                          these commands in YAML that drifts from it.
#
# Legs: audit clippy fmt loc test tts udeps wasm tauri
#
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

# Set by the dispatcher at the bottom. A leg invoked by name is gating a tree
# someone proposed, so its cargo commands take --locked: a pull request that
# needs a different lockfile has to commit one, rather than have the runner
# quietly resolve a fresh dependency tree and green-light *that* instead. The
# local pass leaves this empty, because it opens by rewriting the lockfile on
# purpose — and `cargo upgrade` followed by the fork-manifest restore below
# leaves the lock legitimately ahead of those two manifests, which --locked
# would reject. Always expanded unquoted, so empty contributes no argument.
LOCKED=""

# --- Legs ---------------------------------------------------------------------

# Dependency refresh, and the reason the bare run cannot itself be the CI gate:
# it rewrites the lockfile and every manifest's version requirements, so as a
# pull request check it would test a dependency tree nobody proposed and pass a
# tree nobody would get. Local-only, deliberately.
ci_deps () {
  cargo update --verbose
  cargo upgrade --verbose
  git checkout -- hygg-cff-parser/Cargo.toml hygg-pdf-extract/Cargo.toml
}

ci_audit () {
  cargo audit
}

ci_clippy () {
  cargo +nightly clippy --workspace "${HOST_ONLY[@]}" $LOCKED \
    --all-targets --all-features -- -D warnings
}

# The leg checks; the local pass writes (see `ci` below). Nightly is not a
# preference here: rustfmt.toml leans on `ignore` — the only thing keeping the
# vendored forks unformatted — and on `wrap_comments`, and both are nightly-only
# options that stable rustfmt drops with a warning while reformatting the forks.
ci_fmt () {
  cargo +nightly fmt --all --check
}

# Source hygiene: no authored .rs file may exceed the LOC budget.
ci_loc () {
  tools/loc-gate.sh
}

ci_test () {
  cargo +nightly test --workspace "${HOST_ONLY[@]}" $LOCKED
}

# TTS narration is feature-gated, so the default test run compiles its
# phonemize/alignment regression tests out. Run them explicitly to guard the
# espeak punctuation -> Kokoro pause-token contract across dep bumps (the
# real-espeak test self-locates the build-vendored espeak-ng-data).
#
# Its own leg rather than a second line in ci_test because the feature's deps are
# exactly what set it apart: espeak-rs builds vendored espeak-ng through CMake
# and rodio links ALSA, so on a runner this leg needs system packages that no
# other leg here does.
ci_tts () {
  cargo +nightly test -p cli-text-reader --features tts --lib $LOCKED
}

ci_udeps () {
  cargo +nightly udeps --workspace "${HOST_ONLY[@]}" $LOCKED --all-targets
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
  cargo +nightly clippy -p hygg-tauri $LOCKED --all-targets -- -D warnings
  cargo +nightly build -p hygg-tauri $LOCKED
  # Native extraction commands (base64 decode + the txt/pdf/epub pipeline).
  cargo +nightly test -p hygg-tauri $LOCKED
}

# Browser/wasm leg for hygg-pwa. Compiles + lints the PWA for wasm32, confirms
# the Trunk bundle assembles, and guards the `cargo install hygg` isolation
# invariant (the CLI's native dependency tree must never pull the PWA's
# Leptos/wasm stack).
ci_wasm () {
  rustup +nightly target add wasm32-unknown-unknown >/dev/null 2>&1 || true

  cargo +nightly clippy -p hygg-pwa --target wasm32-unknown-unknown $LOCKED --all-features -- -D warnings
  cargo +nightly build -p hygg-pwa --target wasm32-unknown-unknown $LOCKED
  cargo +nightly udeps -p hygg-pwa --target wasm32-unknown-unknown $LOCKED
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

# --- The local pass -----------------------------------------------------------

# Every gate CI runs, plus the steps a runner must not take: the dependency
# refresh above, and the two below that write to the tree rather than fail on it.
ci () {
  ci_deps
  ci_audit

  cargo +nightly check --workspace "${HOST_ONLY[@]}"
  cargo +nightly fix --allow-dirty --workspace "${HOST_ONLY[@]}"
  ci_clippy
  cargo +nightly fmt --all

  # Run after fmt so the line counts reflect canonical formatting.
  ci_loc

  ci_test
  ci_tts
  ci_udeps

  ci_wasm
  ci_tauri
}

# --- Dispatch -----------------------------------------------------------------

LEGS="audit clippy fmt loc test tts udeps wasm tauri"

if [ $# -eq 0 ]; then
  ci
  exit
fi

for leg in $LEGS; do
  if [ "$1" = "$leg" ]; then
    LOCKED="--locked"
    "ci_$1"
    exit
  fi
done

echo "usage: ${0##*/} [$(echo "$LEGS" | tr ' ' '|')]" >&2
echo "       (no argument runs the full local pass, including the mutating steps)" >&2
exit 2
