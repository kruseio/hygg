#!/usr/bin/env bash

# Deps
# cargo install --locked cargo-audit cargo-edit cargo-udeps cargo-geiger cargo-crev cargo-deny

set -Eeuo pipefail

# hygg-cff-parser and hygg-pdf-extract are verbatim forks of upstream crates,
# kept unmodified so they can be re-synced from upstream with a one-line
# manifest change. They still build (and are type-checked) as dependencies of
# cli-pdf-to-text, but are excluded from the mutating / linting tooling below:
#   - --exclude works for the cargo built-ins (check/fix/clippy/test) + udeps
#   - cargo fmt has no --exclude, so it skips them via rustfmt.toml's `ignore`
#   - cargo upgrade has no per-member exclude (its --exclude filters by
#     dependency name), so the fork manifests are restored right after it runs
FORKS=(--exclude hygg-cff-parser --exclude hygg-pdf-extract)

ci () {
  cargo update --verbose
  cargo upgrade --verbose
  git checkout -- hygg-cff-parser/Cargo.toml hygg-pdf-extract/Cargo.toml
  cargo audit

  cargo +nightly check --workspace "${FORKS[@]}"
  cargo +nightly fix --allow-dirty --workspace "${FORKS[@]}"
  cargo +nightly clippy --workspace "${FORKS[@]}" --all-targets --all-features -- -D warnings
  cargo +nightly fmt --all
  cargo +nightly test --workspace "${FORKS[@]}"

  cargo +nightly udeps --workspace "${FORKS[@]}" --all-targets
  # cargo udeps --all-targets
}

ci
