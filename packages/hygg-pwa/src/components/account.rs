//! Settings → Account: connect this browser to a sync server. You can **create
//! an account** or **sign in** with your email and password — each is exchanged
//! once for a device token, and only that token is stored (the password is
//! never persisted) — or paste a **device token** made in the server's Devices
//! page, the same model as the CLI's `:auth`. Optional throughout — the reader
//! works fully offline without ever connecting.
//!
//! On the official hosted service this section also hosts the subscription flow
//! ([`SubscribeSection`]); a self-host build never shows any commerce.

use leptos::prelude::*;
use leptos::task::spawn_local;

use hygg_shared::sync::proto::MeResponse;

use crate::app::SettingsCtx;
use crate::sync;

use super::account_connect::ConnectForms;
use super::subscribe::SubscribeSection;

/// One-line explanation of what an auto-sync scope covers, shown under the
/// selector.
fn scope_hint(scope: hygg_shared::sync::AutoSyncPolicy) -> &'static str {
  use hygg_shared::sync::AutoSyncPolicy;
  match scope {
    AutoSyncPolicy::All => "Every document syncs across your devices.",
    AutoSyncPolicy::Books => {
      "Books sync automatically. Add other documents from their menu."
    }
    AutoSyncPolicy::Manual => "Only documents you add from their menu sync.",
  }
}

/// The account line shown once connected: whatever label the server supplied,
/// verbatim. A server that sends none (the ordinary case) shows nothing rather
/// than inventing a word for it.
pub(super) fn account_label(me: &MeResponse) -> String {
  me.label.clone().filter(|l| !l.is_empty()).unwrap_or_default()
}

#[component]
pub fn AccountSection() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let account = RwSignal::new(String::new());

  // When already connected, confirm the stored credentials still work and show
  // the account label the server supplied.
  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    if let Some(creds) = settings.with(|s| s.creds()) {
      spawn_local(async move {
        match sync::fetch_me(&creds).await {
          Ok(me) => account.set(account_label(&me)),
          Err(e) => account.set(format!("Reconnect needed: {e}")),
        }
      });
    }
  });

  let disconnect = move |_| {
    settings.update(|s| {
      s.username = None;
      s.api_token = None;
      s.device_id = None;
    });
    settings.with(|s| s.save());
    account.set(String::new());
  };

  view! {
    <section class="setting">
      <label>"Account"</label>
      {move || if settings.with(|s| s.is_connected()) {
        view! {
          <div class="setting__row">
            <span class="account-ok">"Connected for sync"</span>
            <button class="chip" on:click=disconnect>"Disconnect"</button>
          </div>
          {move || {
            let a = account.get();
            (!a.is_empty()).then(|| view! { <p class="setting__hint">{a}</p> })
          }}
          <label class="toggle">
            <input type="checkbox"
              prop:checked=move || settings.with(|s| s.sync_enabled)
              on:change=move |ev| {
                let on = event_target_checked(&ev);
                settings.update(|s| s.sync_enabled = on);
                settings.with(|s| s.save());
              }/>
            <span>"Sync with this server"</span>
          </label>
          {move || settings.with(|s| s.sync_enabled).then(|| view! {
            <label class="modal__label" for="autosync-scope">
              "Auto-sync which documents"
            </label>
            <select id="autosync-scope" class="modal__sync"
              prop:value=move || {
                settings.with(|s| s.auto_sync_scope.as_str().to_string())
              }
              on:change=move |ev| {
                let scope = hygg_shared::sync::AutoSyncPolicy
                  ::from_token_or_default(&event_target_value(&ev));
                settings.update(|s| s.auto_sync_scope = scope);
                settings.with(|s| s.save());
              }>
              <option value="all">"Everything"</option>
              <option value="books">"Books"</option>
              <option value="manual">"Manual"</option>
            </select>
            <p class="setting__hint">
              {move || scope_hint(settings.with(|s| s.auto_sync_scope))}
            </p>
          })}
          <SubscribeSection/>
        }.into_any()
      } else {
        view! { <ConnectForms account=account/> }.into_any()
      }}
    </section>
  }
}
