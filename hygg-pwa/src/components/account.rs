//! Settings → Account: connect this browser to a sync server by entering your
//! **username** and a **device token** (created in the server's Devices page or
//! via the API) — same model as the CLI's `:auth <username> <token>`. No
//! password in the PWA. Both are validated against `/me`, which also binds this
//! browser's machine id to the token and yields the device id + plan. Optional
//! throughout — the reader works fully offline without ever connecting.

use leptos::prelude::*;
use leptos::task::spawn_local;

use hygg_shared::sync::proto::MeResponse;

use crate::app::SettingsCtx;
use crate::sync;

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
fn account_label(me: &MeResponse) -> String {
  me.label.clone().filter(|l| !l.is_empty()).unwrap_or_default()
}

#[component]
pub fn AccountSection() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let user_input = RwSignal::new(String::new());
  let token_input = RwSignal::new(String::new());
  let status = RwSignal::new(String::new());
  let busy = RwSignal::new(false);
  let account = RwSignal::new(String::new());

  // When already connected, confirm the stored credentials still work and show
  // the account's plan.
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

  let connect = move |_| {
    let username = user_input.get().trim().to_string();
    let token = token_input.get().trim().to_string();
    if username.is_empty() || token.is_empty() {
      status.set("Enter your username and device token.".to_string());
      return;
    }
    // Ensure a stable machine id for this browser and build the full
    // credentials (the token binds to this machine on the server).
    let mut machine_id = String::new();
    settings.update(|s| machine_id = s.ensure_machine_id());
    settings.with(|s| s.save());
    let server = settings.with(|s| s.server_url.clone());
    let creds = sync::Creds {
      server,
      token: token.clone(),
      username: username.clone(),
      machine_id,
    };
    busy.set(true);
    status.set("Connecting\u{2026}".to_string());
    spawn_local(async move {
      // Validate username + token and capture the device id + plan from /me.
      match sync::fetch_me(&creds).await {
        Ok(me) => {
          settings.update(|s| {
            s.username = Some(username.clone());
            s.api_token = Some(token.clone());
            s.device_id = Some(me.device_id.clone());
          });
          settings.with(|s| s.save());
          token_input.set(String::new());
          user_input.set(String::new());
          account.set(account_label(&me));
          status.set("Connected.".to_string());
        }
        Err(e) => status.set(format!("Invalid username or token: {e}")),
      }
      busy.set(false);
    });
  };

  let disconnect = move |_| {
    settings.update(|s| {
      s.username = None;
      s.api_token = None;
      s.device_id = None;
    });
    settings.with(|s| s.save());
    account.set(String::new());
    status.set(String::new());
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
        }.into_any()
      } else {
        view! {
          <input class="setting__text" type="text" placeholder="Username"
            autocomplete="username" autocapitalize="off" spellcheck="false"
            prop:value=move || user_input.get()
            on:input=move |ev| user_input.set(event_target_value(&ev))/>
          <input class="setting__text" type="text" placeholder="Device token"
            autocomplete="off" autocapitalize="off" spellcheck="false"
            prop:value=move || token_input.get()
            on:input=move |ev| token_input.set(event_target_value(&ev))/>
          <button class="btn btn--primary" prop:disabled=move || busy.get()
            on:click=connect>"Connect"</button>
          <p class="setting__hint">
            "Create a device token in the server\u{2019}s Devices page (or via the
             API), then enter your username and paste the token here."
          </p>
        }.into_any()
      }}
      {move || {
        let s = status.get();
        (!s.is_empty()).then(|| view! { <p class="setting__hint">{s}</p> })
      }}
    </section>
  }
}
