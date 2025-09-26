#!/usr/bin/env bash

set -Eeuo pipefail

cargo publish -p hygg-shared
cargo publish -p cli-pdf-to-text
cargo publish -p cli-epub-to-text
cargo publish -p cli-justify
cargo publish -p cli-text-reader
cargo publish -p redirect-stderr
# cargo publish -p cli-text-reader-online
cargo publish -p hygg

# cargo publish -p hygg-server
