//! The Account section's disconnected state: create an account, sign in, or
//! paste a device token. Every path ends the same way — a device token stored,
//! validated by `/me`, the account label shown — so the connected view takes
//! over. When a password is used it is sent once to mint the token and never
//! persisted; only the token is kept.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::SettingsCtx;
use crate::sync;

use super::account::account_label;

/// Which connect method the forms are showing.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Create,
  SignIn,
  Token,
}

/// Persist a freshly issued device token + account, then confirm it with `/me`
/// to capture the label. Shared by all three connect paths; the connected view
/// appears reactively as soon as the token lands in settings.
fn finish_connection(
  settings: SettingsCtx,
  account: RwSignal<String>,
  username: String,
  token: String,
  device_id: String,
) {
  settings.update(|s| {
    s.username = Some(username);
    s.api_token = Some(token);
    s.device_id = Some(device_id);
  });
  settings.with(|s| s.save());
  if let Some(creds) = settings.with(|s| s.creds()) {
    spawn_local(async move {
      if let Ok(me) = sync::fetch_me(&creds).await {
        account.set(account_label(&me));
      }
    });
  }
}

/// Ensure this browser has a machine id (persisting it) and return it plus the
/// configured server — the two things every connect path needs up front.
fn machine_and_server(settings: SettingsCtx) -> (String, String) {
  let mut machine_id = String::new();
  settings.update(|s| machine_id = s.ensure_machine_id());
  settings.with(|s| s.save());
  let server = settings.with(|s| s.server_url.clone());
  (machine_id, server)
}

/// Create an account (`create`) or sign a new device into an existing one, from
/// email + password. Both exchange the password for a device token in one call.
fn start_password_connect(
  create: bool,
  settings: SettingsCtx,
  account: RwSignal<String>,
  email: RwSignal<String>,
  password: RwSignal<String>,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let em = email.get().trim().to_string();
  let pw = password.get();
  if em.is_empty() || pw.is_empty() {
    status.set("Enter your email and password.".to_string());
    return;
  }
  let (machine_id, server) = machine_and_server(settings);
  busy.set(true);
  status.set(
    if create { "Creating account\u{2026}" } else { "Signing in\u{2026}" }
      .to_string(),
  );
  spawn_local(async move {
    let issued = if create {
      sync::signup(&server, &em, &pw, &machine_id)
        .await
        .map(|r| (r.token, r.device_id))
    } else {
      sync::register_device(&server, &em, &pw, &machine_id)
        .await
        .map(|r| (r.token, r.device_id))
    };
    match issued {
      Ok((token, device_id)) => {
        password.set(String::new());
        email.set(String::new());
        finish_connection(settings, account, em, token, device_id);
      }
      Err(e) => {
        let what =
          if create { "Could not create account" } else { "Could not sign in" };
        status.set(format!("{what}: {e}"));
      }
    }
    busy.set(false);
  });
}

/// Connect with a device token minted elsewhere (server Devices page / API),
/// keyed on the account username — the same path the CLI's `:auth` takes.
fn start_token_connect(
  settings: SettingsCtx,
  account: RwSignal<String>,
  email: RwSignal<String>,
  token_input: RwSignal<String>,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let username = email.get().trim().to_string();
  let token = token_input.get().trim().to_string();
  if username.is_empty() || token.is_empty() {
    status.set("Enter your username and device token.".to_string());
    return;
  }
  let (machine_id, server) = machine_and_server(settings);
  let creds =
    sync::Creds { server, token, username: username.clone(), machine_id };
  busy.set(true);
  status.set("Connecting\u{2026}".to_string());
  spawn_local(async move {
    match sync::fetch_me(&creds).await {
      Ok(me) => {
        token_input.set(String::new());
        email.set(String::new());
        finish_connection(
          settings,
          account,
          username,
          creds.token,
          me.device_id,
        );
      }
      Err(e) => status.set(format!("Invalid username or token: {e}")),
    }
    busy.set(false);
  });
}

#[component]
pub fn ConnectForms(account: RwSignal<String>) -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let mode = RwSignal::new(Mode::Create);
  let email = RwSignal::new(String::new());
  let password = RwSignal::new(String::new());
  let token_input = RwSignal::new(String::new());
  let status = RwSignal::new(String::new());
  let busy = RwSignal::new(false);

  let tab = move |m: Mode, label: &'static str| {
    view! {
      <button class="chip"
        class=("chip--on", move || mode.get() == m)
        prop:disabled=move || busy.get()
        on:click=move |_| { mode.set(m); status.set(String::new()); }>
        {label}
      </button>
    }
  };

  view! {
    <div class="setting__row setting__row--tabs">
      {tab(Mode::Create, "Create account")}
      {tab(Mode::SignIn, "Sign in")}
    </div>

    {move || (mode.get() != Mode::Token).then(|| {
      let create = move || mode.get() == Mode::Create;
      view! {
        <input class="setting__text" type="email" placeholder="Email"
          autocomplete="email" autocapitalize="off" spellcheck="false"
          prop:value=move || email.get()
          on:input=move |ev| email.set(event_target_value(&ev))/>
        <input class="setting__text" type="password" placeholder="Password"
          autocomplete=move || if create() { "new-password" } else { "current-password" }
          prop:value=move || password.get()
          on:input=move |ev| password.set(event_target_value(&ev))/>
        <button class="btn btn--primary" prop:disabled=move || busy.get()
          on:click=move |_| start_password_connect(
            create(), settings, account, email, password, status, busy)>
          {move || if create() { "Create account" } else { "Sign in" }}
        </button>
        <p class="setting__hint">
          <a href="#" on:click=move |ev| {
            ev.prevent_default();
            mode.set(Mode::Token);
            status.set(String::new());
          }>"Have a device token instead?"</a>
        </p>
      }
    })}

    {move || (mode.get() == Mode::Token).then(|| view! {
      <input class="setting__text" type="text" placeholder="Username"
        autocomplete="username" autocapitalize="off" spellcheck="false"
        prop:value=move || email.get()
        on:input=move |ev| email.set(event_target_value(&ev))/>
      <input class="setting__text" type="text" placeholder="Device token"
        autocomplete="off" autocapitalize="off" spellcheck="false"
        prop:value=move || token_input.get()
        on:input=move |ev| token_input.set(event_target_value(&ev))/>
      <button class="btn btn--primary" prop:disabled=move || busy.get()
        on:click=move |_| start_token_connect(
          settings, account, email, token_input, status, busy)>"Connect"</button>
      <p class="setting__hint">
        "Create a device token in the server\u{2019}s Devices page (or via the
         API), then enter your username and paste the token here. "
        <a href="#" on:click=move |ev| {
          ev.prevent_default();
          mode.set(Mode::Create);
          status.set(String::new());
        }>"Back"</a>
      </p>
    })}

    {move || {
      let s = status.get();
      (!s.is_empty()).then(|| view! { <p class="setting__hint">{s}</p> })
    }}
  }
}
