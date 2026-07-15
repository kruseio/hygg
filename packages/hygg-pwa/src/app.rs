//! Root component: global settings context, theme application, and the
//! touch-first route shell (Home / Reader / Settings).

use std::borrow::Cow;

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::InstallPrompt;
use crate::github::GithubStars;
use crate::routes::{About, Credits, Home, Reader, SettingsView};
use crate::settings::Settings;

/// Reactive settings shared app-wide via context.
pub type SettingsCtx = RwSignal<Settings>;

thread_local! {
  /// Computed once — a document's base URL cannot change after load.
  static BASE: String = base_path();
}

/// The path this bundle is served under, so one build runs at "/" (the Tauri
/// shell, `trunk serve`), at "/hygg/" (the latest Pages deploy) and at
/// "/hygg/v0.1.21/" (a pinned one). Empty at the root, else no trailing slash
/// ("/hygg/v0.1.21"), which is both what leptos_router wants for `base` — it
/// matches it as a plain path prefix — and what `link` can concatenate onto.
///
/// Read from the document's *base* URL, not its location: on a deep link like
/// /hygg/v0.1.21/settings the location names the route, while baseURI resolves
/// the <base href> each Pages deploy carries (injected by
/// tools/prepare_pages_dist.py). With no <base> the two coincide at "/", which
/// is the Tauri and dev-server case.
fn base_path() -> String {
  let base_uri = web_sys::window()
    .and_then(|w| w.document())
    .and_then(|d| d.base_uri().ok().flatten())
    .unwrap_or_default();
  // Absolute ("https://host/hygg/v0.1.21/") — keep the path, drop scheme+host.
  let path = base_uri
    .split_once("://")
    .and_then(|(_, rest)| rest.find('/').map(|i| &rest[i..]))
    .unwrap_or("");
  path.trim_end_matches('/').to_owned()
}

fn router_base() -> Cow<'static, str> {
  BASE.with(|base| Cow::Owned(base.clone()))
}

/// An in-app link target — always route this through here rather than writing
/// `<A href="/settings">` directly.
///
/// leptos_router hands any href starting with "/" straight through: `<Router
/// base>` applies only to *relative* hrefs, and those resolve against the
/// current route rather than the deploy root, so neither form survives a move
/// to /hygg/v0.1.21/. Prefixing here is what keeps a link inside its own
/// deploy.
pub fn link(path: &str) -> String {
  BASE.with(|base| format!("{base}{path}"))
}

#[component]
pub fn App() -> impl IntoView {
  let settings: SettingsCtx = RwSignal::new(Settings::load());
  provide_context(settings);
  // Shared star count for the top-bar pill and the About page. Lazy — no
  // network request until a component that shows stars asks for it.
  provide_context(GithubStars::new());

  // Reflect the chosen theme onto <html> so the stylesheet can theme globally.
  Effect::new(move |_| {
    let theme = settings.read().theme;
    if let Some(doc) = web_sys::window().and_then(|w| w.document())
      && let Some(root) = doc.document_element()
    {
      root.set_class_name(theme.css_class());
    }
  });

  view! {
    <Router base=router_base()>
      <Routes fallback=|| view! { <p class="pad">"Not found."</p> }>
        <Route path=path!("/") view=Home/>
        <Route path=path!("/read/:id") view=Reader/>
        <Route path=path!("/settings") view=SettingsView/>
        <Route path=path!("/about") view=About/>
        <Route path=path!("/credits") view=Credits/>
      </Routes>
      <InstallPrompt/>
    </Router>
  }
}
