#!/usr/bin/env bash
set -euo pipefail

if (( $# != 0 )); then
  echo "Usage: $0" >&2
  exit 2
fi

cd "$(dirname "$0")/.."

if [[ "$(cargo llvm-cov --version 2>/dev/null || true)" != "cargo-llvm-cov 0.8.4" ]]; then
  echo "Install cargo-llvm-cov 0.8.4: cargo install cargo-llvm-cov --version 0.8.4 --locked" >&2
  exit 1
fi

report=target/llvm-cov/justify-summary.json
mkdir -p target/llvm-cov
cargo +nightly-2026-03-05 llvm-cov --branch -q -p cli-justify \
  --json --summary-only --output-path "$report" --locked -j1 \
  -- --test-threads=1

python3 - "$report" <<'PY'
import json
import subprocess
import sys

files = json.load(open(sys.argv[1], encoding="utf-8"))["data"][0]["files"]
changed = subprocess.check_output(
    ["git", "diff", "--name-only", "origin/main...HEAD"], text=True
).splitlines()
for path in changed:
    if not path.startswith("packages/cli-justify/src/") or not path.endswith(".rs"):
        continue
    match = next((item for item in files if item["filename"].endswith("/" + path)), None)
    if match is None:
        print(f"{path}: no coverage data")
        continue
    summary = match["summary"]
    lines = summary["lines"]
    branches = summary["branches"]
    print(
        f"{path.removeprefix('packages/cli-justify/src/')}: "
        f"lines {lines['covered']}/{lines['count']}, "
        f"branches {branches['covered']}/{branches['count']}"
    )
PY
