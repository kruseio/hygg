//! Mobile-app style top bar: a back affordance on the left, the title in the
//! middle, a settings gear on the right. Slides out of view when `visible` is
//! false (the reader hides it on scroll-down for distraction-free reading).

use leptos::prelude::*;
use leptos_router::components::A;

use crate::app::link;

#[component]
pub fn TopBar(
  /// Center title text.
  #[prop(into)]
  title: Signal<String>,
  /// Whether the bar is shown (animates out when false).
  #[prop(into)]
  visible: Signal<bool>,
  /// Where the back chevron links; `None` hides it (e.g. on Home).
  #[prop(optional, into)]
  back_href: Option<String>,
  /// Extra action buttons for the right zone, rendered just left of the
  /// settings gear (e.g. the reader's read-aloud toggle). `None` on screens
  /// with no page-specific actions.
  #[prop(optional)]
  children: Option<Children>,
) -> impl IntoView {
  view! {
    <header class="topbar" class:topbar--hidden=move || !visible.get()>
      <div class="topbar__left">
        {back_href.map(|href| view! {
          <A href=href attr:class="iconbtn" attr:aria-label="Back">
            {chevron_left()}
          </A>
        })}
      </div>
      <div class="topbar__title">{move || title.get()}</div>
      <div class="topbar__right">
        {children.map(|c| c())}
        <A href=link("/settings") attr:class="iconbtn" attr:aria-label="Settings">
          {gear()}
        </A>
      </div>
    </header>
  }
}

fn chevron_left() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="26" height="26" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <polyline points="15 18 9 12 15 6"/>
    </svg>
  }
}

fn gear() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="24" height="24" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
  }
}
