#!/usr/bin/env python3
"""Turn a Trunk `dist/` into a GitHub Pages deploy rooted at a sub-path.

`trunk build --public-url /hygg/v0.1.21/` rewrites the URLs Trunk itself emits
(the hashed wasm/js/css) to absolute ones, but it does *not* emit a <base href>,
and it leaves hand-written relative refs — manifest, icons, the sw.js
registration — untouched. Those resolve against the document, which is fine at
/hygg/v0.1.21/ and wrong at /hygg/v0.1.21/settings. So two fixes are needed:

  1. inject <base href="{public_url}"> so every relative ref (and the router's
     idea of its own root, read from document.baseURI) resolves to the deploy
     root regardless of how deep the current route is;
  2. copy index.html to 404.html — Pages serves static files only, so a hard
     load of a client-side route would otherwise 404. Pages returns 404.html for
     any unmatched path, with the deploy's own <base> already in place.

Usage:  prepare_pages_dist.py <dist-dir> <public-url>
Example: prepare_pages_dist.py packages/hygg-pwa/dist /hygg/v0.1.21/
"""

import re
import shutil
import sys
from pathlib import Path

BASE_TAG = re.compile(r"<base\b[^>]*>", re.IGNORECASE)
HEAD_OPEN = re.compile(r"<head\b[^>]*>", re.IGNORECASE)
# `<head>` is an optional tag: an HTML minifier is allowed to drop it (its
# children then belong to an implicit head). Trunk's release minifier does
# exactly this — our source `<head>` carries no attributes, so it is elided,
# while `<html lang=... class=...>` is kept because it does. When that happens
# the <base> goes right after `<html ...>`, which is where the implicit head
# begins, so it is still the first thing the head resolves against.
HTML_OPEN = re.compile(r"<html\b[^>]*>", re.IGNORECASE)
# Regions where a tag-shaped string is text, not an element: an HTML comment, or
# the body of a <script>/<style>.
INERT = re.compile(
    r"<!--.*?-->|<script\b.*?</script\s*>|<style\b.*?</style\s*>",
    re.DOTALL | re.IGNORECASE,
)


def _first_element(pattern: re.Pattern, html: str):
    """First match of `pattern` that is real markup rather than prose.

    index.html explains its relative-href scheme in both an HTML comment and a JS
    comment, and both spell out the <base> tag. Without skipping inert regions
    the injector rewrites one of those comments and silently produces a page with
    no <base> at all — which looks fine until a deep link 404s.
    """
    inert = [m.span() for m in INERT.finditer(html)]
    for m in pattern.finditer(html):
        if not any(start <= m.start() < end for start, end in inert):
            return m
    return None


def inject_base(html: str, public_url: str) -> str:
    """Put <base href="{public_url}"> first inside <head>.

    First matters: a <base> only governs the refs that follow it, and the
    document's baseURI comes from the first <base> in tree order.
    """
    tag = f'<base href="{public_url}">'
    existing = _first_element(BASE_TAG, html)
    if existing:  # idempotent: re-running retargets rather than stacking tags
        return html[: existing.start()] + tag + html[existing.end() :]
    # Prefer just inside <head>; fall back to just after <html ...> when a
    # minifier dropped the optional <head> tag. Both put <base> ahead of every
    # ref it must govern.
    anchor = _first_element(HEAD_OPEN, html) or _first_element(HTML_OPEN, html)
    if not anchor:
        raise SystemExit(
            "no <head> or <html> in index.html — cannot inject <base>"
        )
    return html[: anchor.end()] + "\n    " + tag + html[anchor.end() :]


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {Path(sys.argv[0]).name} <dist-dir> <public-url>")
    dist, public_url = Path(sys.argv[1]), sys.argv[2]

    # A <base href> without a trailing slash drops its last segment when
    # resolving ("/hygg/v1" + "sw.js" -> "/hygg/sw.js"), which would silently
    # point a pinned deploy at its neighbour's assets.
    if not public_url.endswith("/"):
        public_url += "/"

    index = dist / "index.html"
    if not index.is_file():
        raise SystemExit(f"{index} not found — run `trunk build` first")

    index.write_text(inject_base(index.read_text(), public_url))
    shutil.copyfile(index, dist / "404.html")
    print(f"prepared {dist} for {public_url} (base injected, 404.html written)")


if __name__ == "__main__":
    main()
