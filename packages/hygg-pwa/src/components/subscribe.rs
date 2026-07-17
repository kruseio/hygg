//! Settings → Account → Subscribe: the dormant subscription surface.
//!
//! Renders nothing unless the app is pointed at the official hosted service
//! ([`crate::settings::Settings::is_official_server`]) *and* the server returns
//! plans. A self-host build compiles this, but the guard keeps it invisible —
//! no plan, no price, no upsell ever appears off the official service.
//!
//! "Subscribe" asks the server for a one-time checkout handoff URL and
//! navigates there; the payment itself happens on the server's own origin (the
//! PWA never touches card details).

use leptos::prelude::*;
use leptos::task::spawn_local;

use hygg_shared::sync::proto::CommercePlan;

use crate::app::SettingsCtx;
use crate::sync;

/// Navigate to a checkout handoff URL in the same tab. A pop-up opened after
/// the async ticket call would be blocked, so we replace the current document.
fn go_to(url: &str) {
  if let Some(loc) = web_sys::window().map(|w| w.location()) {
    let _ = loc.set_href(url);
  }
}

/// Human price for a plan, e.g. `5.00 USD/mo`, or `Free`.
fn price(plan: &CommercePlan) -> String {
  if plan.price_cents == 0 {
    "Free".to_string()
  } else {
    format!(
      "{}.{:02} {}/mo",
      plan.price_cents / 100,
      plan.price_cents % 100,
      plan.currency
    )
  }
}

#[component]
pub fn SubscribeSection() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  // Off the official service the whole surface is inert.
  if !settings.with(|s| s.is_official_server()) {
    return ().into_any();
  }

  let plans = RwSignal::new(Vec::<CommercePlan>::new());
  let status = RwSignal::new(String::new());
  let busy = RwSignal::new(false);

  // Discover what the server sells. A 404 / error leaves the list empty and the
  // section renders nothing.
  let server = settings.with(|s| s.server_url.clone());
  spawn_local(async move {
    if let Ok(found) = sync::fetch_plans(&server).await {
      plans.set(found);
    }
  });

  let subscribe = move |slug: String| {
    let Some(creds) = settings.with(|s| s.creds()) else {
      status.set("Connect an account first.".to_string());
      return;
    };
    busy.set(true);
    status.set("Opening checkout\u{2026}".to_string());
    spawn_local(async move {
      match sync::start_checkout(&creds, &slug).await {
        Ok(url) => go_to(&url),
        Err(e) => {
          status.set(format!("Could not start checkout: {e}"));
          busy.set(false);
        }
      }
    });
  };

  view! {
    {move || {
      (!plans.get().is_empty()).then(|| view! {
        <p class="setting__hint">"Subscription"</p>
        <div class="subscribe-list">
          <For
            each=move || plans.get()
            key=|p| p.slug.clone()
            children=move |p| {
              let slug = p.slug.clone();
              let label = format!("{} \u{2014} {}", p.name, price(&p));
              view! {
                <div class="setting__row">
                  <span>{label}</span>
                  <button class="btn btn--primary" prop:disabled=move || busy.get()
                    on:click=move |_| subscribe(slug.clone())>"Subscribe"</button>
                </div>
              }
            }/>
        </div>
      })
    }}
    {move || {
      let s = status.get();
      (!s.is_empty()).then(|| view! { <p class="setting__hint">{s}</p> })
    }}
  }
  .into_any()
}
