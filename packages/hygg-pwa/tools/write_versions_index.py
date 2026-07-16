#!/usr/bin/env python3
"""Write versions.json + versions.html listing every pinned PWA deploy.

The Pages site keeps one directory per tag (0.1.21/, 0.1.22/, ...) alongside the
latest build at the root. This enumerates them newest-first so a pinned version
is discoverable without knowing tag names by heart.

Usage: write_versions_index.py <site-dir> <base-url>
Example: write_versions_index.py site /hygg/
"""

import json
import re
import sys
from pathlib import Path

# The `v` is optional because this repository's tags are bare — 0.1.22, not
# v0.1.22. Requiring it matched none of the directories pages.yml actually
# writes, so every pinned deploy was left out of both files this generates.
VERSION_DIR = re.compile(r"^v?(\d+)\.(\d+)\.(\d+)(?:[-.](.+))?$")


def sort_key(name: str):
    """Newest first, numerically. A pre-release (v1.0.0-rc1) sorts *below* the
    release it precedes, matching semver."""
    m = VERSION_DIR.match(name)
    major, minor, patch, pre = int(m[1]), int(m[2]), int(m[3]), m[4]
    # No pre-release sorts above any pre-release of the same version.
    return (major, minor, patch, pre is None, pre or "")


PAGE = """<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>hygg — versions</title>
    <style>
      :root {{ color-scheme: dark light; }}
      body {{
        margin: 0 auto; padding: 2.5rem 1.25rem; max-width: 34rem;
        font: 16px/1.6 ui-monospace, SFMono-Regular, Menlo, monospace;
        background: #0b0b0b; color: #e8e8e8;
      }}
      @media (prefers-color-scheme: light) {{
        body {{ background: #fdfdfd; color: #1a1a1a; }}
      }}
      a {{ color: inherit; }}
      h1 {{ font-size: 1.25rem; font-weight: 600; margin: 0 0 .25rem; }}
      p {{ opacity: .7; margin: 0 0 2rem; }}
      ul {{ list-style: none; padding: 0; }}
      li {{ padding: .6rem 0; border-bottom: 1px solid rgba(128,128,128,.25); }}
      .tag {{ font-size: .75rem; opacity: .6; margin-left: .5rem; }}
    </style>
  </head>
  <body>
    <h1>hygg — pinned versions</h1>
    <p>
      <a href="{base}">{base}</a> always serves the latest release. Each version
      below stays put, so a link to one keeps working.
    </p>
    <ul>
{items}
    </ul>
  </body>
</html>
"""


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(f"usage: {Path(sys.argv[0]).name} <site-dir> <base-url>")
    site, base = Path(sys.argv[1]), sys.argv[2]
    if not base.endswith("/"):
        base += "/"

    versions = sorted(
        (p.name for p in site.iterdir() if p.is_dir() and VERSION_DIR.match(p.name)),
        key=sort_key,
        reverse=True,
    )

    (site / "versions.json").write_text(
        json.dumps({"latest": versions[0] if versions else None,
                    "versions": versions}, indent=2) + "\n"
    )

    items = "\n".join(
        f'      <li><a href="{base}{v}/">{v}</a>'
        f'{"<span class=tag>latest</span>" if i == 0 else ""}</li>'
        for i, v in enumerate(versions)
    )
    (site / "versions.html").write_text(PAGE.format(base=base, items=items))
    print(f"indexed {len(versions)} version(s): {', '.join(versions) or '(none)'}")


if __name__ == "__main__":
    main()
