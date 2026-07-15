//! Bake build-time metadata into hygg-gui:
//!
//! 1. **Git provenance** — the commit this binary was built from (short + full
//!    sha and its date), so the in-app About screen and the macOS "About" panel
//!    can show exactly which build a user is running. Best-effort: a build
//!    without git falls back to "unknown" at the use site (`build_info.rs`).
//! 2. **The default sync-server URL** from `packages/hygg-gui/.env`, so a
//!    self-host / LAN build can point at its own server without hardcoding (or
//!    committing) an address. Unset → the SaaS default. Mirrors
//!    `hygg-pwa/build.rs`. An explicit environment variable takes precedence
//!    over the `.env` file.
//! 3. **A Windows VERSIONINFO resource** (Windows host only) so the exe's
//!    Properties → Details tab shows the version/publisher/copyright/commit —
//!    the Windows counterpart of the macOS About panel.

use std::path::Path;
use std::process::Command;

fn main() {
  emit_git_info();
  emit_server_url();
  emit_windows_resource();
}

/// Embed a Windows VERSIONINFO resource into `hygg-gui.exe` so its Properties →
/// Details tab (Windows' closest thing to the macOS "About" panel) shows the
/// version, publisher, copyright, and the commit it was built from. Only runs
/// on a Windows host targeting Windows; a no-op — and not even compiled —
/// elsewhere. A missing resource compiler is a warning, never a build failure.
#[cfg(windows)]
fn emit_windows_resource() {
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    return;
  }
  let version = env!("CARGO_PKG_VERSION");
  let sha = git(&["rev-parse", "--short=9", "HEAD"])
    .unwrap_or_else(|| "unknown".to_string());
  let sha_full = git(&["rev-parse", "HEAD"]).unwrap_or_default();
  let repo = env!("CARGO_PKG_REPOSITORY");

  let part = |k: &str| -> u64 {
    std::env::var(k).ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(0)
      as u64
  };
  let ver = (part("CARGO_PKG_VERSION_MAJOR") << 48)
    | (part("CARGO_PKG_VERSION_MINOR") << 32)
    | (part("CARGO_PKG_VERSION_PATCH") << 16);

  let mut res = winresource::WindowsResource::new();
  res
    .set("ProductName", "hygg")
    .set("FileDescription", "hygg — a calm, offline-first document reader")
    .set("CompanyName", "kruseio")
    .set("LegalCopyright", "© kruseio — AGPL-3.0-only")
    .set("OriginalFilename", "hygg-gui.exe")
    .set("ProductVersion", &format!("{version} ({sha})"))
    .set("Comments", &format!("commit {sha_full}; {repo}"))
    .set_version_info(winresource::VersionInfo::FILEVERSION, ver)
    .set_version_info(winresource::VersionInfo::PRODUCTVERSION, ver);
  if let Err(e) = res.compile() {
    println!(
      "cargo:warning=hygg-gui: Windows version resource not embedded: {e}"
    );
  }
}

#[cfg(not(windows))]
fn emit_windows_resource() {}

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

/// Bake `HYGG_GUI_SERVER_URL` from `packages/hygg-gui/.env` (unless already set
/// in the environment) so `Settings::default` can read it via `option_env!`.
fn emit_server_url() {
  let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
  println!("cargo:rerun-if-changed={}", env_path.display());
  println!("cargo:rerun-if-env-changed=HYGG_GUI_SERVER_URL");

  if std::env::var("HYGG_GUI_SERVER_URL").is_ok() {
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
      && key.trim() == "HYGG_GUI_SERVER_URL"
    {
      let value = value.trim().trim_matches('"').trim_matches('\'');
      if !value.is_empty() {
        println!("cargo:rustc-env=HYGG_GUI_SERVER_URL={value}");
      }
    }
  }
}
