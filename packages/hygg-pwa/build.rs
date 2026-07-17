//! Bake two kinds of build-time metadata into hygg-pwa:
//!
//! 1. **Git provenance** — the commit this bundle was built from (short + full
//!    sha and its date), so the in-app About page can show exactly which build
//!    is running. Best-effort: a build without git falls back to "unknown" at
//!    the use site (`build_info.rs`).
//! 2. **The default server URL** from `packages/hygg-pwa/.env`. A local /
//!    self-host dev build can point at a LAN server by setting
//!    `HYGG_PWA_SERVER_URL` in `packages/hygg-pwa/.env` (gitignored) — this
//!    emits it as a compile-time env var that `Settings::default` reads via
//!    `option_env!`, so no address is hardcoded (or committed) in the source.
//!    Unset → the hosted default. An explicit environment variable takes
//!    precedence over the `.env` file.

use std::path::Path;
use std::process::Command;

fn main() {
  emit_git_info();
  emit_server_url();
}

/// Emit the commit sha (short + full) and date as compile-time env vars, and
/// ask cargo to re-run when HEAD moves so a new commit re-bakes them.
fn emit_git_info() {
  if let Some(v) = git(&["rev-parse", "--short=9", "HEAD"]) {
    println!("cargo:rustc-env=HYGG_GIT_SHA={v}");
  }
  if let Some(v) = git(&["rev-parse", "HEAD"]) {
    println!("cargo:rustc-env=HYGG_GIT_SHA_FULL={v}");
  }
  if let Some(v) = git(&["log", "-1", "--format=%cI"]) {
    println!("cargo:rustc-env=HYGG_GIT_DATE={v}");
  }
  // `logs/HEAD` changes on every commit, `HEAD` on branch switch — watching
  // both keeps the baked sha honest without forcing a rebuild every time.
  for rel in ["HEAD", "logs/HEAD"] {
    if let Some(p) = git(&["rev-parse", "--git-path", rel]) {
      println!("cargo:rerun-if-changed={p}");
    }
  }
}

/// Run a `git` command in the crate directory, returning its trimmed stdout, or
/// `None` when git is missing / this isn't a checkout.
fn git(args: &[&str]) -> Option<String> {
  let out = Command::new("git")
    .args(args)
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .output()
    .ok()?;
  if !out.status.success() {
    return None;
  }
  let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
  (!s.is_empty()).then_some(s)
}

/// Bake `HYGG_PWA_SERVER_URL` from `packages/hygg-pwa/.env` (unless already set
/// in the environment) so `Settings::default` can read it via `option_env!`.
fn emit_server_url() {
  let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
  println!("cargo:rerun-if-changed={}", env_path.display());
  println!("cargo:rerun-if-env-changed=HYGG_PWA_SERVER_URL");

  // A real environment variable already reaches `option_env!`; don't override.
  if std::env::var("HYGG_PWA_SERVER_URL").is_ok() {
    return;
  }
  let Ok(contents) = std::fs::read_to_string(&env_path) else {
    return;
  };
  for line in contents.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
      continue;
    }
    if let Some((key, value)) = line.split_once('=')
      && key.trim() == "HYGG_PWA_SERVER_URL"
    {
      let value = value.trim().trim_matches('"').trim_matches('\'');
      if !value.is_empty() {
        println!("cargo:rustc-env=HYGG_PWA_SERVER_URL={value}");
      }
    }
  }
}
