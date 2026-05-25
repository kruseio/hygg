#!/usr/bin/env bash

# Deps
# cargo install --locked cargo-audit cargo-edit cargo-udeps cargo-geiger cargo-crev cargo-deny

set -Eeuo pipefail

ci () {
  cargo update --verbose
  cargo upgrade --verbose
  cargo audit

  cargo +nightly check
  cargo +nightly fix --allow-dirty
  cargo +nightly clippy --all-targets --all-features -- -D warnings
  cargo +nightly fmt --all
  cargo +nightly test

  cargo +nightly udeps --all-targets
  # cargo udeps --all-targets
}

ci
