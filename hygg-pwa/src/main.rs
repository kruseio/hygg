//! hygg-pwa — Rust/WASM offline-first touch reader.
//!
//! ## One crate, two build targets
//!
//! Built for `wasm32-unknown-unknown` (what Trunk compiles) this is the
//! client-side rendered (CSR) Leptos app mounted to `<body>`: all document
//! parsing/justification reuses the hygg pipeline crates compiled to wasm, and
//! the reader chrome is touch-first (no vim, no command line).
//!
//! Built for the host target (`cargo run -p hygg-pwa`) it is instead a tiny
//! launcher that shells out to Trunk, so the whole dev/build flow is one cargo
//! command with no `cd hygg-pwa && trunk …` to remember. The Leptos/web-sys
//! stack is `cfg(target_arch = "wasm32")`-gated in `Cargo.toml`, so the host
//! build pulls none of it — the launcher needs only `std` to spawn `trunk`.

#[cfg(target_arch = "wasm32")]
mod ansi;
#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod assets;
#[cfg(target_arch = "wasm32")]
mod build_info;
#[cfg(target_arch = "wasm32")]
mod clock;
#[cfg(target_arch = "wasm32")]
mod components;
#[cfg(target_arch = "wasm32")]
mod format;
#[cfg(target_arch = "wasm32")]
mod layout;
#[cfg(target_arch = "wasm32")]
mod model;
#[cfg(target_arch = "wasm32")]
mod routes;
#[cfg(target_arch = "wasm32")]
mod settings;
#[cfg(target_arch = "wasm32")]
mod sse;
#[cfg(target_arch = "wasm32")]
mod storage;
#[cfg(target_arch = "wasm32")]
mod sync;
#[cfg(target_arch = "wasm32")]
mod tauri_ipc;
#[cfg(target_arch = "wasm32")]
mod tts;

#[cfg(target_arch = "wasm32")]
fn main() {
  console_error_panic_hook::set_once();
  leptos::mount::mount_to_body(app::App);
}

/// Host-target launcher: `cargo run -p hygg-pwa [-- <trunk args>]`.
///
/// With no extra args it runs `trunk serve` — the hot-reloading dev server,
/// with address/port taken from `Trunk.toml` (127.0.0.1:8080). Anything after
/// `--` is forwarded verbatim, so `cargo run -p hygg-pwa -- build --release`
/// emits the production bundle in `./dist`. Trunk is invoked from this crate's
/// directory so it finds `Trunk.toml`/`index.html` no matter the caller's cwd.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
  use std::process::{Command, ExitCode};

  let mut args: Vec<String> = std::env::args().skip(1).collect();
  if args.is_empty() {
    // The common case: the hot-reloading dev server.
    args.push("serve".to_string());
  }

  match Command::new("trunk")
    .args(&args)
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .status()
  {
    Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      eprintln!(
        "hygg-pwa: `trunk` was not found on PATH.\n\
         Install it with:  cargo install --locked trunk\n\
         It needs the wasm target: rustup target add wasm32-unknown-unknown"
      );
      ExitCode::FAILURE
    }
    Err(err) => {
      eprintln!("hygg-pwa: failed to launch trunk: {err}");
      ExitCode::FAILURE
    }
  }
}
