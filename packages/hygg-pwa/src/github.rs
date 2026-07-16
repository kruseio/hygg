//! Live GitHub repository stats: the star count, fetched from the public
//! GitHub API at most once per app load and shared app-wide via context (the
//! top-bar pill and the About page both read the same signal). Best-effort —
//! offline or rate-limited the count simply stays `None` and callers render
//! without a number.

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

use crate::build_info as bi;

/// Shared, lazily fetched star count. Nothing hits the network until the
/// first component that actually shows stars calls [`GithubStars::ensure`],
/// so a user who disables the top-bar pill and never opens About costs no
/// request.
#[derive(Clone, Copy)]
pub struct GithubStars {
  count: RwSignal<Option<u32>>,
  started: StoredValue<bool>,
}

impl GithubStars {
  pub fn new() -> Self {
    GithubStars { count: RwSignal::new(None), started: StoredValue::new(false) }
  }

  /// Kick off the fetch if it hasn't started yet (idempotent).
  pub fn ensure(self) {
    if self.started.get_value() {
      return;
    }
    self.started.set_value(true);
    let count = self.count;
    spawn_local(async move {
      if let Ok(n) = fetch_star_count().await {
        count.set(Some(n));
      }
    });
  }

  /// Reactive read of the star count (`None` until fetched).
  pub fn count(self) -> Option<u32> {
    self.count.get()
  }
}

/// Just the one field we need from the repo endpoint.
#[derive(Deserialize)]
struct Repo {
  stargazers_count: u32,
}

/// Fetch the repo's current star count. The browser supplies the User-Agent
/// GitHub requires, so no auth or header juggling is needed.
async fn fetch_star_count() -> Result<u32, String> {
  let url = format!("https://api.github.com/repos/{}/{}", bi::OWNER, bi::REPO);
  let resp = Request::get(&url)
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(format!("GitHub returned {}", resp.status()));
  }
  let repo: Repo = resp.json().await.map_err(|e| e.to_string())?;
  Ok(repo.stargazers_count)
}

/// Compact count for tight UI: "982", "1.4k", "12k".
pub fn format_count(n: u32) -> String {
  if n < 1_000 {
    n.to_string()
  } else if n < 10_000 {
    format!("{:.1}k", n as f32 / 1000.0)
  } else {
    format!("{}k", n / 1000)
  }
}

/// Filled star glyph, tinted via `currentColor`. This is GitHub's own
/// `octicon-star-fill` path (16×16 grid), so the top-bar Star button and the
/// About page read as faithful reproductions of the button on the repo page.
pub fn star_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 16 16" width="16" height="16" fill="currentColor"
      aria-hidden="true">
      <path d="M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 \
        .416 1.279l-3.046 2.97.719 4.192a.751.751 0 0 1-1.088.791L8 \
        12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 \
        0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Z"/>
    </svg>
  }
}
