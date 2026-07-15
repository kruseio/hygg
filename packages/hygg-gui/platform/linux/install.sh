#!/usr/bin/env bash
# Install hygg-gui on Linux (GNOME/XDG) as a document reader and, optionally, the
# default handler for PDF / EPUB / text. Installs into the per-user prefix so no
# root is needed.
#
# Usage:  packages/hygg-gui/platform/linux/install.sh [--default]
#   --default   also set hygg as the default app for its MIME types
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE="$(cd "$HERE/../.." && pwd)"
# The crate lives at packages/hygg-gui, so the workspace root — the directory
# holding Cargo.toml and target/ — is two levels above it, not one.
ROOT="$(cd "$CRATE/../.." && pwd)"

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/512x512/apps"
METAINFO_DIR="$HOME/.local/share/metainfo"

echo "› building hygg-gui (release)…"
( cd "$ROOT" && cargo build -p hygg-gui --release )

install -Dm755 "$ROOT/target/release/hygg-gui" "$BIN_DIR/hygg-gui"
install -Dm644 "$HERE/hygg-gui.desktop" "$APP_DIR/hygg-gui.desktop"
install -Dm644 "$CRATE/assets/icons/icon-512.png" "$ICON_DIR/hygg-gui.png"

# AppStream metadata — the Linux equivalent of the macOS "About" panel. GNOME
# Software / KDE Discover show the version, description, developer, and links
# from it. Substitute the live workspace version + commit date (best-effort).
VERSION="$(grep -m1 -E '^version = ' "$ROOT/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')"
VERSION="${VERSION:-0.0.0}"
DATE="$(git -C "$ROOT" log -1 --format=%cs 2>/dev/null || date +%F)"
META_TMP="$(mktemp)"
sed -e "s/__HYGG_VERSION__/$VERSION/g" -e "s/__HYGG_DATE__/$DATE/g" \
  "$HERE/com.kruseio.hygg-gui.metainfo.xml" > "$META_TMP"
install -Dm644 "$META_TMP" "$METAINFO_DIR/com.kruseio.hygg-gui.metainfo.xml"
rm -f "$META_TMP"

# Refresh the desktop + icon caches so the launcher and "Open With" pick it up.
command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" || true
command -v gtk-update-icon-cache >/dev/null &&
  gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "✓ installed hygg-gui → $BIN_DIR/hygg-gui (version $VERSION)"

if [[ "${1:-}" == "--default" ]]; then
  if command -v xdg-mime >/dev/null; then
    for mime in application/pdf application/epub+zip text/plain text/markdown; do
      xdg-mime default hygg-gui.desktop "$mime"
      echo "  set default for $mime"
    done
  else
    echo "  xdg-mime not found — skipping default-handler setup"
  fi
fi

cat <<EOF

Make sure $BIN_DIR is on your PATH. Then either double-click a PDF in Files
(Nautilus → right-click → Open With → hygg) or run:
  hygg-gui ~/Documents/book.pdf
EOF
