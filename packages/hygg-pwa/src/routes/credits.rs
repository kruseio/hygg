//! The Credits page: the author (with their GitHub avatar), every repository
//! contributor pulled live from GitHub, a mock "Buy me a coffee" support
//! button, and a shortcut back to Settings. The fetch is best-effort — offline
//! it shows the author card and a gentle note. Mirrors the native GUI Credits
//! screen.

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use serde::Deserialize;

use super::about::github_icon;
use crate::app::link;
use crate::build_info as bi;
use crate::components::TopBar;

/// Placeholder donation link behind the mock "Buy me a coffee" button. TODO:
/// swap for the real Buy Me a Coffee page once the account is set up.
const SUPPORT_URL: &str = "https://www.buymeacoffee.com/kruseio";

/// One GitHub contributor, from the repo contributors API.
#[derive(Clone, Debug, Deserialize)]
struct Contributor {
  login: String,
  #[serde(default)]
  avatar_url: String,
  #[serde(default)]
  html_url: String,
  #[serde(default)]
  contributions: u32,
}

/// Load state for the contributor list.
#[derive(Clone)]
enum Load {
  Loading,
  Ready(Vec<Contributor>),
  Failed(String),
}

#[component]
pub fn Credits() -> impl IntoView {
  let state = RwSignal::new(Load::Loading);
  // Fetch the contributor list once, on mount.
  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    spawn_local(async move {
      state.set(match fetch_contributors().await {
        Ok(list) => Load::Ready(list),
        Err(e) => Load::Failed(e),
      });
    });
  });

  let author_avatar = format!("https://github.com/{}.png?size=160", bi::OWNER);
  let author_url = format!("https://github.com/{}", bi::OWNER);

  view! {
    <TopBar
      title=Signal::derive(|| "Credits".to_string())
      visible=Signal::derive(|| true)
      back_href=link("/settings")
    />
    <main class="credits">
      <section class="panel credits__author">
        <img class="avatar avatar--lg" src=author_avatar alt=bi::AUTHOR
          loading="lazy"/>
        <div>
          <h2>{bi::AUTHOR}</h2>
          <p class="muted">"Author & maintainer"</p>
          <a class="link" href=author_url target="_blank" rel="noopener">
            {github_icon()} {format!("github.com/{}", bi::OWNER)}
          </a>
        </div>
      </section>

      <section class="panel credits__support">
        <h3>"Support the project"</h3>
        <p class="muted">
          "hygg is free and open source. If it makes your reading calmer, you \
           can chip in for a coffee."
        </p>
        <a class="btn btn--primary coffee" href=SUPPORT_URL target="_blank"
          rel="noopener">
          {coffee_icon()} "Buy me a coffee"
        </a>
      </section>

      <section class="panel">
        <h3>"Contributors"</h3>
        {move || match state.get() {
          Load::Loading => view! {
            <p class="muted">"Loading contributors\u{2026}"</p>
          }.into_any(),
          Load::Failed(e) => view! {
            <p class="muted">
              {format!("Couldn't load contributors ({e}). They'll appear once online.")}
            </p>
          }.into_any(),
          Load::Ready(list) if list.is_empty() => view! {
            <p class="muted">"No contributors found yet."</p>
          }.into_any(),
          Load::Ready(list) => view! {
            <ul class="contributors">
              {list.into_iter().map(contributor_item).collect::<Vec<_>>()}
            </ul>
          }.into_any(),
        }}
      </section>

      <div class="credits__footer">
        <A href=link("/settings") attr:class="btn">"Open settings"</A>
        <A href=link("/about") attr:class="btn">"About hygg"</A>
      </div>
    </main>
  }
}

/// One contributor cell: a round avatar over their login, linking to their
/// GitHub profile; the commit count rides along as the hover title.
fn contributor_item(c: Contributor) -> impl IntoView {
  let title = format!("{} \u{00b7} {} commits", c.login, c.contributions);
  view! {
    <li>
      <a href=c.html_url target="_blank" rel="noopener" title=title>
        <img class="avatar" src=c.avatar_url alt=c.login.clone() loading="lazy"/>
        <span>{c.login}</span>
      </a>
    </li>
  }
}

/// Fetch the repo's contributors (most-contributions first). Bots / anonymous
/// rows are dropped so the grid shows real people. The browser supplies the
/// User-Agent GitHub requires, so no auth or header juggling is needed.
async fn fetch_contributors() -> Result<Vec<Contributor>, String> {
  let url = format!(
    "https://api.github.com/repos/{}/{}/contributors?per_page=100",
    bi::OWNER,
    bi::REPO
  );
  let resp = Request::get(&url)
    .header("Accept", "application/vnd.github+json")
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(format!("GitHub returned {}", resp.status()));
  }
  let mut list: Vec<Contributor> =
    resp.json().await.map_err(|e| e.to_string())?;
  list.retain(|c| !c.login.is_empty() && !c.login.ends_with("[bot]"));
  list.sort_by_key(|c| std::cmp::Reverse(c.contributions));
  Ok(list)
}

/// Coffee cup glyph, tinted via `currentColor`.
fn coffee_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="18" height="18" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round" aria-hidden="true">
      <path d="M18 8h1a4 4 0 0 1 0 8h-1"/>
      <path d="M2 8h16v9a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4V8z"/>
      <line x1="6" y1="1" x2="6" y2="4"/>
      <line x1="10" y1="1" x2="10" y2="4"/>
      <line x1="14" y1="1" x2="14" y2="4"/>
    </svg>
  }
}
