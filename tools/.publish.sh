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
# The three clients (hygg-gui, hygg-pwa, hygg-tauri) are `publish = false` and
# are deliberately absent.
#
# Since Rust 1.66 `cargo publish` waits for the index to propagate before it
# returns, so these need no sleeps between them.

set -Eeuo pipefail

# Leaves: nothing in the workspace stands under them.
cargo publish -p hygg-shared
cargo publish -p cli-justify
cargo publish -p cli-image-to-ascii
cargo publish -p redirect-stderr
cargo publish -p hygg-cff-parser

# One layer up.
cargo publish -p cli-epub-to-text # hygg-shared
cargo publish -p hygg-pdf-extract # hygg-cff-parser

# cli-image-to-ascii, cli-justify, hygg-pdf-extract, hygg-shared, redirect-stderr
cargo publish -p cli-pdf-to-text

# cli-justify, cli-pdf-to-text, hygg-shared
cargo publish -p cli-text-reader

# Top of the tree. hygg-server is here because it is `publish = true` and
# consumable as a library (hygg-saas embeds it) — nothing in hygg's tree
# references it, and no license crosses between them.
cargo publish -p hygg        # + cli-epub-to-text, cli-text-reader, redirect-stderr
cargo publish -p hygg-server # cli-epub-to-text, cli-justify, cli-pdf-to-text, hygg-shared
