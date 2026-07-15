#!/usr/bin/env bash

# Publish every publishable workspace crate to crates.io, dependencies first.
#
# The order is load-bearing. `cargo publish` strips the `path` off a dependency
# and keeps only its version requirement, so each crate's verification build
# resolves its siblings *from the registry* — publish a crate before something it
# depends on and it is built against whatever is already up there. Every
# requirement here is a caret `"0.1"`, so a stale sibling still satisfies it and
# the mistake stays silent: it surfaces the day a crate uses an API added to a
# sibling in the same release, and then it fails partway through a run, with the
# earlier crates already pushed and immutable.
#
# The two clients (hygg-pwa, hygg-tauri) are `publish = false` and
# are deliberately absent.
#
# Re-running is safe: a crate already on crates.io at this version is skipped
# rather than retried. That is what lets .github/workflows/publish.yml run this
# on a tag, and it is what makes a half-finished release finishable. When
# hygg-server's packaging was broken it sat at 0.1.15 while the ten crates ahead
# of it reached 0.1.22 — and because a published version is immutable, every
# attempt to push the one crate that still needed it died on the first crate that
# did not. Skipping turns that from a dead end into a re-run.
#
# Since Rust 1.66 `cargo publish` waits for the index to propagate before it
# returns, so these need no sleeps between them.

set -Eeuo pipefail

# `cargo publish -p` and the version read below both want the workspace root, so
# anchor there rather than trusting the caller's directory.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Every member takes `version.workspace = true`, so [workspace.package]'s version
# is the version of all of them, and the first `version =` in the root manifest.
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || {
  echo "could not read the workspace version from Cargo.toml" >&2
  exit 1
}

echo "publishing hygg $VERSION to crates.io"

# crates.io answers 200 for a version that exists and 404 for one that does not,
# which is the whole check. It refuses requests that send no User-Agent, hence
# the header.
published () {
  curl -sf -o /dev/null \
    -H "User-Agent: hygg-publish (https://github.com/kruseio/hygg)" \
    "https://crates.io/api/v1/crates/$1/$VERSION"
}

publish () {
  if published "$1"; then
    echo "-- $1 $VERSION is already on crates.io — skipping"
    return
  fi
  echo "-- publishing $1 $VERSION"
  cargo publish -p "$1"
}

# Leaves: nothing in the workspace stands under them.
publish hygg-shared
publish cli-justify
publish cli-image-to-ascii
publish redirect-stderr
publish hygg-cff-parser

# One layer up.
publish cli-epub-to-text # hygg-shared
publish hygg-pdf-extract # hygg-cff-parser

# cli-image-to-ascii, cli-justify, hygg-pdf-extract, hygg-shared, redirect-stderr
publish cli-pdf-to-text

# cli-justify, cli-pdf-to-text, hygg-shared
publish cli-text-reader

# Top of the tree. hygg-server is here because it is `publish = true` and
# consumable as a library (hygg-saas embeds it) — nothing in hygg's tree
# references it, and no license crosses between them.
publish hygg        # + cli-epub-to-text, cli-text-reader, redirect-stderr
publish hygg-server # cli-epub-to-text, cli-justify, cli-pdf-to-text, hygg-shared
