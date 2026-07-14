#!/usr/bin/env bash
# Cargo `runner` for macOS (wired up in .cargo/config.toml).
#
# Cargo can't sign a binary in build.rs — build scripts run *before* linking, so
# the binary doesn't exist yet. The next-best hook is the `runner`, which cargo
# invokes to *execute* a freshly built binary (on `cargo run` / `cargo test`)
# with the binary path as the first argument. That runs after linking, so we can
# sign here.
#
# Why sign at all: rustc/cargo leave Mach-O binaries "linker-signed" — a
# placeholder ad-hoc signature macOS treats as unsigned. With the Application
# Firewall on, inbound LAN connections to such a binary are blocked (localhost
# still works; loopback is never firewalled). Replacing it with a real ad-hoc
# signature — exactly what the .NET SDK does to its apphost on every build —
# makes the firewall treat it like any normal app.
#
# Only the server binary is signed; every other binary (the reader, test
# harnesses, …) is exec'd unchanged, so this is a no-op passthrough for them.
set -euo pipefail

bin="$1"
shift

case "$bin" in
  */hygg-server) codesign --force --sign - "$bin" >/dev/null 2>&1 || true ;;
esac

exec "$bin" "$@"
