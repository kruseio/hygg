#!/usr/bin/env bash
# Build hygg-gui.app — a double-clickable macOS bundle that registers hygg as a
# document handler (so you can set it as your default PDF / EPUB / text reader in
# Finder → Get Info → Open with → Change All).
#
# Usage:  hygg-gui/platform/macos/bundle.sh [--release]
# Output: hygg-gui/hygg-gui.app
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE="$(cd "$HERE/../.." && pwd)"
ROOT="$(cd "$CRATE/.." && pwd)"

PROFILE_DIR="release"
CARGO_FLAGS="--release"
if [[ "${1:-}" == "--debug" ]]; then
  PROFILE_DIR="debug"
  CARGO_FLAGS=""
fi

echo "› building hygg-gui ($PROFILE_DIR)…"
( cd "$ROOT" && cargo build -p hygg-gui $CARGO_FLAGS )
BIN="$ROOT/target/$PROFILE_DIR/hygg-gui"

APP="$CRATE/hygg-gui.app"
echo "› assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/hygg-gui"

# --- build provenance for the standard "About hygg" panel -------------------
# The version comes from the workspace (single source of truth); the commit
# details from git (best-effort). These feed both the Info.plist version and a
# Resources/Credits.html, which macOS shows in the About panel's scroll area.
VERSION="$(grep -m1 -E '^version = ' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
VERSION="${VERSION:-0.0.0}"
COMMIT="$(git -C "$ROOT" rev-parse --short=9 HEAD 2>/dev/null || echo unknown)"
COMMIT_FULL="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo '')"
COMMIT_DATE="$(git -C "$ROOT" log -1 --format=%cI 2>/dev/null | cut -dT -f1 || echo '')"
REPO_URL="https://github.com/kruseio/hygg"
COMMIT_URL="$REPO_URL"
[[ -n "$COMMIT_FULL" ]] && COMMIT_URL="$REPO_URL/commit/$COMMIT_FULL"
COMMITTED=""
[[ -n "$COMMIT_DATE" ]] && COMMITTED=" ($COMMIT_DATE)"

# Substitute the version placeholders while copying the plist template.
sed "s/__HYGG_VERSION__/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# The About panel renders Credits.html (or .rtf) from Resources automatically.
cat > "$APP/Contents/Resources/Credits.html" <<HTML
<!DOCTYPE html>
<html><head><meta charset="utf-8"><style>
  body { font: 12px -apple-system, "Helvetica Neue", sans-serif; color: #4a4a4a; margin: 0; }
  a { color: #b07d3a; text-decoration: none; }
  .k { color: #999; display: inline-block; min-width: 66px; }
  p { margin: 3px 0; }
  .foot { color: #999; margin-top: 8px; }
</style></head><body>
  <p>A calm, offline-first document reader.</p>
  <p><span class="k">Version</span> $VERSION</p>
  <p><span class="k">Commit</span> <a href="$COMMIT_URL">$COMMIT</a>$COMMITTED</p>
  <p><span class="k">Author</span> kruseio</p>
  <p><span class="k">Source</span> <a href="$REPO_URL">$REPO_URL</a></p>
  <p class="foot">© kruseio · AGPL-3.0-only</p>
</body></html>
HTML
echo "› About panel: version $VERSION, commit $COMMIT"

# Generate the .icns from the shipped 512px icon (best-effort; skipped if the
# Apple icon tools aren't available).
if command -v sips >/dev/null && command -v iconutil >/dev/null; then
  ICONSET="$(mktemp -d)/hygg.iconset"
  mkdir -p "$ICONSET"
  SRC="$CRATE/assets/icons/icon-512.png"
  for size in 16 32 64 128 256 512; do
    sips -z "$size" "$size" "$SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    dbl=$((size * 2))
    sips -z "$dbl" "$dbl" "$SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/hygg.icns"
  echo "› icon: hygg.icns"
else
  echo "› sips/iconutil not found — bundling without a custom icon"
fi

# Register the bundle with LaunchServices so it appears in "Open With".
LSREG="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
[[ -x "$LSREG" ]] && "$LSREG" -f "$APP" || true

cat <<EOF

✓ Built $APP

Set hygg as your default PDF reader:
  • Finder → right-click any .pdf → Get Info
  • "Open with" → choose hygg → "Change All…"
Or open a document directly:
  open -a "$APP" /path/to/document.pdf
EOF
