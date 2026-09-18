#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )); then
  echo "Usage: $0 [core|toc]" >&2
  exit 2
fi

cd "$(dirname "$0")/.."

if [[ "$(cargo mutants --version 2>/dev/null || true)" != "cargo-mutants 27.1.0" ]]; then
  echo "Install cargo-mutants 27.1.0: cargo install cargo-mutants --version 27.1.0 --locked" >&2
  exit 1
fi

# Keep both mutation and Cargo compilation to one worker. The default scope
# covers Unicode column measurement and wrapping; `toc` covers the parsers and
# compact layout logic touched by this change.
case "${1:-core}" in
  core)
    files=(
      -f packages/cli-justify/src/justify_text.rs
      -f packages/cli-justify/src/text_utils.rs
      -f packages/cli-justify/src/wrap.rs
      -f packages/cli-justify/src/pdf_hybrid/wrapping_plain.rs
      -f packages/cli-justify/src/pdf_hybrid/wrapping/toc.rs
    )
    selector='replace (justify|justify_line|split_at_width|display_width|split_at_last_whitespace_before|drop_one_leading_whitespace|leading_whitespace|leading_whitespace_width|wrap_line_preserving_whitespace|wrap_plain_with_prefix|wrap_aligned_toc_row) ->| in (justify|justify_line|split_at_width|display_width|split_at_last_whitespace_before|drop_one_leading_whitespace|leading_whitespace|leading_whitespace_width|wrap_line_preserving_whitespace|wrap_plain_with_prefix|wrap_aligned_toc_row)$'
    output=()
    report=mutants.out/outcomes.json
    ;;
  toc)
    files=(
      -f packages/cli-justify/src/pdf_hybrid/structure/toc.rs
      -f packages/cli-justify/src/pdf_hybrid/structure/toc_patterns.rs
    )
    selector=' in (parse_aligned_toc_row_start|parse_aligned_toc_continuation|normalize_preserved_compact_layout_line|merge_counter_into_prefix_if_needed)$|replace (parse_aligned_toc_row_start|parse_aligned_toc_continuation|normalize_preserved_compact_layout_line|merge_counter_into_prefix_if_needed) ->'
    output=(-o target/mutation-justify-toc)
    report=target/mutation-justify-toc/mutants.out/outcomes.json
    ;;
  *)
    echo "Usage: $0 [core|toc]" >&2
    exit 2
    ;;
esac

run_started_at=$(date +%s)
mutation_status=0
cargo +1.94.0 mutants -p cli-justify \
  "${files[@]}" --re "$selector" "${output[@]}" \
  --jobs 1 --jobserver-tasks 1 --cargo-arg=-j1 \
  --timeout 6 --build-timeout 120 || mutation_status=$?

if [[ ! -f "$report" ]]; then
  echo "Mutation run produced no outcome report (exit $mutation_status)" >&2
  exit 1
fi

python3 - "$report" "$run_started_at" <<'PY'
import json
import os
import sys

if os.path.getmtime(sys.argv[1]) < int(sys.argv[2]):
    sys.exit("Mutation outcome report is stale")
outcomes = json.load(open(sys.argv[1], encoding="utf-8"))
total = outcomes["total_mutants"]
baseline = next(
    (item for item in outcomes["outcomes"] if item["scenario"] == "Baseline"), None
)
if baseline is None or baseline["summary"] != "Success" or total == 0:
    sys.exit("Mutation baseline failed or no mutants were tested")
if len(outcomes["outcomes"]) != total + 1:
    sys.exit("Mutation run did not finish all mutants")

caught = outcomes["caught"]
print(
    f"Strict mutation score: {caught}/{total} = {100 * caught / total:.1f}% "
    f"({outcomes['missed']} missed, {outcomes['timeout']} timed out, "
    f"{outcomes['unviable']} unviable)"
)
if caught * 10 < total * 9:
    sys.exit("Mutation score is below 90%")
PY
