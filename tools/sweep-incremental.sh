#!/usr/bin/env bash

# Keeps rustc's incremental caches under the cap set by `incremental-max-gb` in
# [workspace.metadata.sweep] in the root Cargo.toml.
#
# Cargo has no setting for this. rustc already discards the older sessions of a
# crate it rebuilds, but every distinct crate hash — a feature set, a target, a
# toolchain — gets a cache of its own, and the ones no build asks for again stay
# on disk until something removes them. That is what grows without bound, and
# what this removes: every target/**/incremental/<crate>-<hash> directory,
# oldest first by last use, until the total fits under the cap. The next build
# that needs one of them recompiles that crate from scratch, and is otherwise
# unaffected.
#
# Run it by hand, or let tools/hooks/pre-push run it before every push. Not
# during a build: deleting a cache rustc has open fails that build.
#
#   tools/sweep-incremental.sh       sweep down to the cap
#   tools/sweep-incremental.sh -n    say what would go, delete nothing

set -Eeuo pipefail

dry_run=false
[ "${1:-}" = "-n" ] && dry_run=true

root="$(cd "$(dirname "$0")/.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"

max_gb="$(sed -n '/^\[workspace\.metadata\.sweep\]/,/^\[/s/^incremental-max-gb *= *\([0-9][0-9]*\).*/\1/p' \
  "$root/Cargo.toml" | head -1)"
[ -n "$max_gb" ] || { echo "sweep: no incremental-max-gb in Cargo.toml" >&2; exit 1; }
max_kb=$((max_gb * 1024 * 1024))

[ -d "$target" ] || exit 0

# Every per-crate cache directory, least recently used first. A crate
# directory's mtime moves whenever rustc starts a new session inside it, so it
# is a last-used time. ls rather than stat, whose flags differ between the BSD
# and GNU builds; no mapfile, which macOS's bash 3.2 does not have.
found=()
while IFS= read -r inc; do
  for d in "$inc"/*/; do
    [ -d "$d" ] && found+=("$d")
  done
done < <(find "$target" -type d -name incremental -prune -print)

[ "${#found[@]}" -gt 0 ] || exit 0

# One ls over all of them, not xargs, which may split the list across several
# calls and sort each part on its own.
caches=()
while IFS= read -r d; do
  caches+=("$d")
done < <(ls -1dtr "${found[@]}")

total_kb=0
for d in "${caches[@]}"; do
  total_kb=$((total_kb + $(du -sk "$d" | cut -f1)))
done

if [ "$total_kb" -le "$max_kb" ]; then
  exit 0
fi

echo "sweep: incremental caches are $((total_kb / 1024)) MB, cap ${max_gb} GB" >&2
for d in "${caches[@]}"; do
  [ "$total_kb" -gt "$max_kb" ] || break
  kb=$(du -sk "$d" | cut -f1)
  if [ "$dry_run" = true ]; then
    echo "sweep: would remove ${d#"$root"/} ($((kb / 1024)) MB)" >&2
  else
    rm -rf "$d"
  fi
  total_kb=$((total_kb - kb))
done
if [ "$dry_run" = true ]; then
  echo "sweep: would be down to $((total_kb / 1024)) MB" >&2
else
  echo "sweep: down to $((total_kb / 1024)) MB" >&2
fi
