#!/usr/bin/env python3
"""Fast self-test for prepare_pages_dist.inject_base — no Trunk build required.

Guards the <base> injector against the failure that broke 0.1.25's Pages deploy:
Trunk's release minifier is allowed to drop the optional <head> tag, and the old
injector could only anchor on a literal <head>, so it aborted with "no <head>".
Cheap enough (pure string work, no wasm compile) to run in tools/ci.sh's `fast`
/ pre-push subset — the full Trunk-build end-to-end check lives in `ci_wasm`.
"""

import importlib.util
from pathlib import Path

_src = Path(__file__).with_name("prepare_pages_dist.py")
_spec = importlib.util.spec_from_file_location("prepare_pages_dist", _src)
_mod = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_mod)
inject_base = _mod.inject_base


def expect(name: str, ok: bool) -> None:
    if not ok:
        raise SystemExit(f"prepare_pages_dist self-test FAILED: {name}")


URL = "/hygg/1.2.3/"
TAG = f'<base href="{URL}">'

# 1. Normal document: <base> lands inside <head>, ahead of the head's content.
normal = (
    '<!DOCTYPE html><html lang="en"><head>'
    "<meta charset=utf-8><title>x</title></head><body>b</body></html>"
)
out = inject_base(normal, URL)
expect("normal: exactly one base", out.count(TAG) == 1)
expect("normal: base after <head>", out.index("<base") > out.index("<head"))
expect("normal: base before <meta>", out.index("<base") < out.index("<meta"))

# 2. Minified: the optional <head> is dropped (the 0.1.25 failure); <html ...>
#    survives because it carries attributes, so <base> anchors after it instead.
minified = (
    '<!DOCTYPE html><html lang="en" class="theme-dark">'
    "<meta charset=utf-8><title>x</title><body>b</body></html>"
)
out = inject_base(minified, URL)
expect("minified: exactly one base", out.count(TAG) == 1)
expect("minified: no literal <head> was invented", "<head" not in out)
expect("minified: base after <html>", out.index("<base") > out.index("<html"))
expect("minified: base before <meta>", out.index("<base") < out.index("<meta"))

# 3. Idempotent: a second pass retargets the existing base instead of stacking.
again = inject_base(out, "/hygg/9.9.9/")
expect("idempotent: still one base", again.count("<base href=") == 1)
expect("idempotent: retargeted", "/hygg/9.9.9/" in again and URL not in again)

# 4. A <base> that appears only inside an HTML comment is inert, not a real tag,
#    so a fresh one is injected rather than the comment being rewritten.
commented = (
    "<!DOCTYPE html><html><head><!-- we inject <base href> here -->"
    "<title>x</title></head><body></body></html>"
)
out = inject_base(commented, URL)
expect("comment: injects a real base despite the inert mention", out.count(TAG) == 1)

print("prepare_pages_dist self-test: all cases passed")
