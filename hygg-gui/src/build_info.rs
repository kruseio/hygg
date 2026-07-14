//! Compile-time build provenance, surfaced by the About / Credits screens and
//! the macOS "About" panel: the crate version plus the git commit this binary
//! was built from (short + full sha and its date), baked by `build.rs`. A build
//! without git (rare — this crate is never published) degrades gracefully to
//! "unknown" / empty rather than failing to compile.

/// Semantic version (workspace `version`, e.g. "0.1.21").
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Canonical source repository, e.g. "https://github.com/kruseio/hygg".
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Project author / maintainer, shown on the Credits page.
pub const AUTHOR: &str = "kruseio";

/// GitHub account that owns the repo (for the contributors API + avatar URLs).
pub const OWNER: &str = "kruseio";

/// GitHub repository name (for the contributors API).
pub const REPO: &str = "hygg";

/// Short commit hash (9 chars), or "unknown" when built outside a git checkout.
pub const GIT_SHA: &str = match option_env!("HYGG_GIT_SHA") {
  Some(s) => s,
  None => "unknown",
};

/// Full 40-char commit hash (empty when unavailable).
pub const GIT_SHA_FULL: &str = match option_env!("HYGG_GIT_SHA_FULL") {
  Some(s) => s,
  None => "",
};

/// ISO-8601 commit date (e.g. "2026-07-05T18:01:33Z"); empty when unavailable.
pub const GIT_DATE: &str = match option_env!("HYGG_GIT_DATE") {
  Some(s) => s,
  None => "",
};

/// A permalink to the exact commit this build came from, falling back to the
/// repo root when the full hash isn't baked in.
pub fn commit_url() -> String {
  if GIT_SHA_FULL.is_empty() {
    REPOSITORY.to_string()
  } else {
    format!("{REPOSITORY}/commit/{GIT_SHA_FULL}")
  }
}

/// The commit timestamp for display: the baked ISO date with the `T` separator
/// swapped for a space (`2026-07-05 18:01:33Z`); empty when unknown.
pub fn commit_timestamp() -> String {
  GIT_DATE.replacen('T', " ", 1)
}
