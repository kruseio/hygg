//! Root component: global settings context, theme application, and the
//! touch-first route shell (Home / Reader / Settings).

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use crate::components::InstallPrompt;
use crate::routes::{About, Credits, Home, Reader, SettingsView};
use crate::settings::Settings;

/// Reactive settings shared app-wide via context.
pub type SettingsCtx = RwSignal<Settings>;

#[component]
pub fn App() -> impl IntoView {
  let settings: SettingsCtx = RwSignal::new(Settings::load());
  provide_context(settings);

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
    <Router>
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
