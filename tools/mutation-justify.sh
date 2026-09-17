#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  echo "Usage: $0" >&2
  exit 2
fi

cd "$(dirname "$0")/.."

if [[ "$(cargo mutants --version 2>/dev/null || true)" != "cargo-mutants 27.1.0" ]]; then
  echo "Install cargo-mutants 27.1.0: cargo install cargo-mutants --version 27.1.0 --locked" >&2
  exit 1
fi

# Keep both mutation and Cargo compilation to one worker. This scope covers
# Unicode column measurement, whitespace handling, and aligned TOC wrapping.
cargo +1.94.0 mutants -p cli-justify \
  -f packages/cli-justify/src/text_utils.rs \
  -f packages/cli-justify/src/wrap.rs \
  -f packages/cli-justify/src/pdf_hybrid/wrapping/toc.rs \
  --re 'replace (split_at_width|display_width|split_at_last_whitespace_before|drop_one_leading_whitespace|leading_whitespace|leading_whitespace_width|wrap_line_preserving_whitespace|wrap_aligned_toc_row) ->| in (split_at_width|display_width|split_at_last_whitespace_before|drop_one_leading_whitespace|leading_whitespace|leading_whitespace_width|wrap_line_preserving_whitespace|wrap_aligned_toc_row)$' \
  --jobs 1 --jobserver-tasks 1 --cargo-arg=-j1 \
  --timeout 6 --build-timeout 120
