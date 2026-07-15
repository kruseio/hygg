#!/usr/bin/env bash

# The workspace check suite, and only ever a check: every command below reads the
# tree and none of them writes to it, so this file is safe to run against work in
# progress, in a hook, or on a runner. The steps that write — the dependency
# refresh, cargo fix, cargo fmt --all — are the maintainer's local pass and live
# in tools/ci-mutable.sh, which sources this file for the pins, the exclusions
# and the legs themselves rather than keeping a second copy of them.
#
#   ./tools/ci.sh          Every leg, cheapest first. The local reproduction of
#                          a full CI run, and as slow as one.
#
#   ./tools/ci.sh fast     fmt, loc, clippy, test — the subset that
#                          tools/hooks/pre-push runs before a branch leaves the
#                          machine. See ci_fast below for why those four, and
#                          why a green hook is not a green CI.
#
#   ./tools/ci.sh <leg>    One gate, alone. This is how .github/workflows/ci.yml
#                          runs — a job per leg — so a pull request into main is
#                          held to *this* file rather than to a second copy of
#                          these commands in YAML that drifts from it.
#
# Legs: audit clippy fmt loc test tts udeps wasm tauri
#
# AGENTS: do not run this file, or any leg of it, unless you were explicitly
# asked to. A full pass compiles the workspace several times over on two
# toolchains, builds a Trunk bundle and a Tauri app, and resolves the advisory
# database; it is measured in tens of minutes, and running it to "check my work"
# after a small edit spends all of that to learn what `cargo check -p <member>`
# would have said in seconds. The one invocation that is not your decision is the
# pre-push hook: if you are pushing on someone's behalf, let it run its four legs
# — reaching for `--no-verify` to get a push through is exactly the thing the
# hook exists to prevent.
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
# cli-pdf-to-text, but are excluded from the mutating / linting tooling here and
# in ci-mutable.sh:
#   - --exclude works for the cargo built-ins (check/fix/clippy/test) + udeps
#   - cargo fmt has no --exclude, so it skips them via rustfmt.toml's `ignore`
#   - cargo upgrade has no per-member exclude (its --exclude filters by
#     dependency name), so ci-mutable.sh restores the fork manifests right after
#     it runs
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

# Set by the dispatcher at the bottom. Every invocation of *this* file is gating
# a tree someone proposed — a pull request, or a push about to become one — so
# its cargo commands take --locked: a tree that needs a different lockfile has to
# commit one, rather than have the runner quietly resolve a fresh dependency tree
# and green-light *that* instead. tools/ci-mutable.sh sources this file and never
# reaches the dispatcher, so it runs the same legs with this left empty — which
# is what it wants, since it opens by rewriting the lockfile on purpose. Always
# expanded unquoted, so empty contributes no argument.
LOCKED=""

# Toolchains are pinned, never floating — and this is the file that pins them,
# for CI, the hook and the local pass alike, so no two of them can disagree by
# accident about what "the compiler" is. ci-mutable.sh disagrees on purpose, and
# says so in one line (see TOOLCHAIN below); it still gets the versions here.
#
# A floating `+nightly` is what put three jobs red on 0.1.23: clippy grew
# `chunks_exact_to_as_chunks` overnight, and a tree that passed yesterday failed
# today having changed nothing. Bumping either line below is then a commit
# someone chose to make and can review, rather than a Tuesday.
#
# Only two legs below name NIGHTLY, and neither is a preference; they provably
# cannot run any other way:
#   - fmt: rustfmt.toml's `ignore` and `wrap_comments` are nightly-only options.
#     Stable rustfmt drops both with a warning and then reformats the vendored
#     forks, which is the single thing `ignore` exists to prevent.
#   - udeps: cargo-udeps passes -Z flags, which stable rustc rejects outright.
# Pinning them buys the same thing pinning STABLE does: rustfmt and clippy both
# rewrite their own rules over time, and neither should do it under a release.
STABLE="+1.94.0"
NIGHTLY="+nightly-2026-03-05"

# The toolchain every other leg runs on — which is every leg that has a choice.
# Stable, because a gate should hold a tree to the compiler that ships rather
# than to the one that might, and a leg that fails here has to name something the
# release will actually hit.
#
# tools/ci-mutable.sh sources this file and points TOOLCHAIN at NIGHTLY, and that
# pass is the one place the flip belongs: it already resolves the dependency tree
# forward, so it is where you want to hear that next month's compiler has an
# opinion — while there is time to answer it, rather than on the morning the pins
# above move. What it finds there is news, not a gate; this file stays the gate.
#
# Expanded unquoted, like LOCKED, and every leg goes through it or through
# NIGHTLY: a bare `cargo` anywhere below would silently run on whatever `rustup
# default` says, which is a machine-wide setting neither CI nor this file can
# see, and is exactly the float the pins exist to close.
TOOLCHAIN="$STABLE"

# --- Legs ---------------------------------------------------------------------

# The one leg with no toolchain on it, and the one leg that needs none:
# cargo-audit compiles nothing. Its inputs are Cargo.lock and the advisory
# database, and the binary that reads them is the same binary whichever cargo
# shim invoked it — so a pin here would change no output, while costing ci.yml's
# audit job a toolchain install it currently does without.
ci_audit () {
  cargo audit
}

ci_clippy () {
  cargo $TOOLCHAIN clippy --workspace "${HOST_ONLY[@]}" $LOCKED \
    --all-targets --all-features -- -D warnings
}

# Checks; ci-mutable.sh is where the same rustfmt writes. See NIGHTLY above for
# why this one leg cannot move to stable.
ci_fmt () {
  cargo $NIGHTLY fmt --all --check
}

# Source hygiene: no authored .rs file may exceed the LOC budget.
ci_loc () {
  tools/loc-gate.sh
}

ci_test () {
  cargo $TOOLCHAIN test --workspace "${HOST_ONLY[@]}" $LOCKED
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
  cargo $TOOLCHAIN test -p cli-text-reader --features tts --lib $LOCKED
}

# Nightly, of necessity — see NIGHTLY above.
ci_udeps () {
  cargo $NIGHTLY udeps --workspace "${HOST_ONLY[@]}" $LOCKED --all-targets
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
  # The Trunk build below is a wasm32 build (see ci_wasm), and it runs on the pin
  # rather than on the default toolchain, so the pin is what needs the target.
  rustup $TOOLCHAIN target add wasm32-unknown-unknown >/dev/null 2>&1 || true

  ( cd packages/hygg-pwa && RUSTUP_TOOLCHAIN="${TOOLCHAIN#+}" trunk build --release )
  cargo $TOOLCHAIN clippy -p hygg-tauri $LOCKED --all-targets -- -D warnings
  cargo $TOOLCHAIN build -p hygg-tauri $LOCKED
  # Native extraction commands (base64 decode + the txt/pdf/epub pipeline).
  cargo $TOOLCHAIN test -p hygg-tauri $LOCKED
}

# Browser/wasm leg for hygg-pwa. Compiles + lints the PWA for wasm32, confirms
# the Trunk bundle assembles, and guards the `cargo install hygg` isolation
# invariant (the CLI's native dependency tree must never pull the PWA's
# Leptos/wasm stack).
ci_wasm () {
  # Both toolchains: the lint/build below run on the pin, the udeps after them
  # cannot, and each needs the target added to its own toolchain. (Under
  # ci-mutable.sh the two are the same toolchain, and the second add is a no-op.)
  rustup $TOOLCHAIN target add wasm32-unknown-unknown >/dev/null 2>&1 || true
  rustup $NIGHTLY target add wasm32-unknown-unknown >/dev/null 2>&1 || true

  cargo $TOOLCHAIN clippy -p hygg-pwa --target wasm32-unknown-unknown $LOCKED --all-features -- -D warnings
  cargo $TOOLCHAIN build -p hygg-pwa --target wasm32-unknown-unknown $LOCKED
  cargo $NIGHTLY udeps -p hygg-pwa --target wasm32-unknown-unknown $LOCKED

  # Trunk shells out to cargo itself, so the pin has to reach it through the
  # environment. Left alone it builds the bundle with whatever `rustup default`
  # says — which on a runner is decided by the order the toolchains were
  # installed in, and on a laptop by a machine-wide setting neither this file nor
  # CI can see. The bundle this ships is not the place to find that out.
  ( cd packages/hygg-pwa && RUSTUP_TOOLCHAIN="${TOOLCHAIN#+}" trunk build --release )

  # Isolation guard: fail if any Leptos/wasm or GUI crate leaks into the
  # published CLI's normal dependency tree (cargo install hygg must never pull
  # the PWA's Leptos/wasm stack or a GUI shell's stack).
  if cargo $TOOLCHAIN tree -p hygg -e normal --prefix none 2>/dev/null \
       | grep -Eiq '^(leptos|gloo|wasm-bindgen|web-sys|js-sys|iced|wgpu|winit|tauri)'; then
    echo "ERROR: hygg dependency tree leaked PWA/GUI/Tauri crates (cargo install hygg must stay clean)" >&2
    exit 1
  fi
}

# --- The passes ---------------------------------------------------------------

# Every leg, ordered by what they cost rather than by what they check: on a
# runner the legs are parallel jobs and the order here is irrelevant, but run
# from a terminal they are serial and someone is watching, and a misformatted
# file should say so in seconds rather than after a Tauri build.
ci_all () {
  ci_fmt
  ci_loc
  ci_audit
  ci_clippy
  ci_test
  ci_tts
  ci_udeps
  ci_wasm
  ci_tauri
}

# What tools/hooks/pre-push runs, and the reason it is a subset rather than the
# suite: the four legs here are the ones whose cost is a compile the laptop was
# going to pay anyway, and they catch most of what turns a pull request red. The
# five they leave out each want something a runner has and a laptop does not —
# espeak built through CMake (tts), a second full build on nightly (udeps), a
# Trunk release bundle (wasm), a WebKitGTK stack and a Tauri binary (tauri) — or,
# in audit's case, an answer that changes with the advisory database rather than
# with the push.
#
# The result is deliberately not a promise that CI will be green. It is the half
# of CI that fails for reasons visible from here, bought at a price people will
# actually pay: a hook that costs a Tauri build is a hook that teaches everyone
# to type --no-verify, and then it gates nothing at all.
ci_fast () {
  ci_fmt
  ci_loc
  ci_clippy
  ci_test
}

# --- Dispatch -----------------------------------------------------------------

# Sourced rather than run — by tools/ci-mutable.sh, for the pins and the legs
# above. Define them and let the caller decide what to run.
if [ "${BASH_SOURCE[0]}" != "$0" ]; then
  return 0
fi

LEGS="audit clippy fmt loc test tts udeps wasm tauri"

if [ $# -eq 0 ]; then
  LOCKED="--locked"
  ci_all
  exit
fi

# Not a leg: prints a pin from the block above so ci.yml can install the exact
# toolchain the legs then invoke. This is the same argument the rest of this
# file makes — a version hard-coded into the YAML as well is a second copy, and
# a second copy drifts. Needs no toolchain itself, so the workflow can call it
# before installing one.
if [ "$1" = "toolchain" ]; then
  case "${2:-}" in
    stable) echo "${STABLE#+}" ;;
    nightly) echo "${NIGHTLY#+}" ;;
    *)
      echo "usage: ${0##*/} toolchain [stable|nightly]" >&2
      exit 2
      ;;
  esac
  exit
fi

# Also not a leg: the hook's subset, named here rather than spelled out as four
# calls in tools/hooks/pre-push, so that what the hook gates is a decision this
# file records and not one buried in .git's plumbing.
if [ "$1" = "fast" ]; then
  LOCKED="--locked"
  ci_fast
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
echo "       ${0##*/} fast    (fmt + loc + clippy + test — what pre-push runs)" >&2
echo "       (no argument runs every leg; none of them write to the tree)" >&2
echo "       (the dependency refresh and the fixups are tools/ci-mutable.sh)" >&2
exit 2
