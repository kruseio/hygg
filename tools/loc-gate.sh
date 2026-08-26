#!/usr/bin/env bash

# LOC gate — fails when any *authored* source file grows past a line budget,
# to keep modules small and reviewable.
#
# "Authored" excludes any file carrying an `@generated` marker in its first few
# lines — an escape hatch for future codegen; there is none in the authored
# crates today. (It also used to exclude two vendored upstream forks, which were
# retired once upstream shipped the fix they carried.)
#
# build output never reaches the check: the file list comes from `git ls-files`
# (tracked + untracked-but-not-gitignored), so target/ is implicitly excluded.
#
# The budget is 300 physical lines; override with LOC_LIMIT, e.g.
#   LOC_LIMIT=400 ./tools/loc-gate.sh

set -Eeuo pipefail

# `git ls-files` below lists only the files under cwd, so the gate would silently
# check nothing but itself if run from tools/. Anchor to the repo root.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

LIMIT="${LOC_LIMIT:-300}"

offenders=()
checked=0

while IFS= read -r -d '' f; do
  [ -f "$f" ] || continue
  # Escape hatch: skip genuinely generated files.
  if head -n 5 -- "$f" | grep -q '@generated'; then continue; fi
  checked=$((checked + 1))
  n=$(wc -l < "$f" | tr -d '[:space:]')
  if [ "$n" -gt "$LIMIT" ]; then
    offenders+=("${n}	${f}")
  fi
done < <(git ls-files -z --cached --others --exclude-standard -- '*.rs')

if [ "${#offenders[@]}" -gt 0 ]; then
  echo "LOC gate: FAILED — authored .rs files must be <= ${LIMIT} lines:" >&2
  printf '%s\n' "${offenders[@]}" | sort -rn | while IFS=$'\t' read -r n f; do
    printf '  %6s  %s\n' "$n" "$f" >&2
  done
  echo >&2
  echo "  ${#offenders[@]} file(s) over ${LIMIT} lines (of ${checked} authored files checked)." >&2
  echo "  Split them into smaller modules, or mark generated files with '@generated'." >&2
  exit 1
fi

echo "LOC gate: OK — all ${checked} authored .rs files <= ${LIMIT} lines."
