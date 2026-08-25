//! The Settings "Encryption" section: a state-aware wizard for end-to-end
//! encryption. Off → offer to turn it on (generating a key). Account-on but
//! this browser has no key → prompt to paste it. Set up → offer to convert
//! existing documents. The heavy lifting lives in
//! [`super::encryption_actions`].

use leptos::prelude::*;

use super::encryption_actions as act;
use crate::app::SettingsCtx;

#[component]
pub fn EncryptionSection() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let status = RwSignal::new(String::new());
  let busy = RwSignal::new(false);
  let generated = RwSignal::new(None::<String>);
  let key_input = RwSignal::new(String::new());

  // On first render, reflect the account's marker into local settings so a
  // browser joining an already-encrypted account is routed to the key prompt.
  act::sync_marker(settings);

  let enabled = move || settings.with(|s| s.encryption_enabled);
  // Whether a key has been published for the account yet (non-empty salt). A
  // server-mandated account is enabled but not yet initialized.
  let initialized = move || {
    settings.with(|s| {
      s.encryption_salt.as_deref().map(|x| !x.is_empty()).unwrap_or(false)
    })
  };
  // "Set up here" requires both a stored secret *and* an initialized marker to
  // derive against — so a stale secret left over after a server-side reset
  // (salt cleared) doesn't masquerade as being set up.
  let has_key =
    move || settings.with(|s| s.encryption_key.is_some()) && initialized();

  view! {
    <section class="setting">
      <label>"Encryption"</label>

      {move || generated.get().map(|phrase| view! {
        <div class="setting__hint" style="border:1px solid var(--accent); \
            padding:0.75rem; border-radius:0.5rem;">
          <strong>"Your encryption key — save it now"</strong>
          <input class="setting__text" type="text" readonly=true
            prop:value=phrase.clone()/>
          <p>"Store this in your password manager. It is the ONLY way to read \
              your documents — lose it and the data is unrecoverable, and the \
              server operator cannot help. Set the same key on every other \
              device (CLI: HYGG_ENCRYPTION_KEY)."</p>
          <button class="btn" prop:disabled=move || busy.get()
            on:click=move |_| generated.set(None)>"I\u{2019}ve saved it"</button>
        </div>
      })}

      // OFF for the account: offer to turn it on.
      {move || (!enabled()).then(|| view! {
        <p class="setting__hint">
          "End-to-end encryption seals your documents and notes on this device \
           before they reach the server, which then stores only unreadable \
           ciphertext. Every device must use the same key."
        </p>
        <button class="btn btn--primary" prop:disabled=move || busy.get()
          on:click=move |_| act::turn_on(settings, generated, status, busy)>
          "Turn on end-to-end encryption"
        </button>
      })}

      // Account REQUIRED by the server but no key created yet: create it here.
      {move || (enabled() && !has_key() && !initialized()).then(|| view! {
        <p class="setting__hint">
          "This account requires end-to-end encryption (set on the server), but \
           no key has been created yet. Create one here \u{2014} then set the \
           same key up on your other devices."
        </p>
        <button class="btn btn--primary" prop:disabled=move || busy.get()
          on:click=move |_| act::turn_on(settings, generated, status, busy)>
          "Create the encryption key"
        </button>
      })}

      // Account ON, this browser set up: healthy state + convert/disable/forget.
      {move || (enabled() && has_key()).then(|| view! {
        <p class="setting__hint">
          "On. Documents and notes are encrypted before they leave this browser."
        </p>
        <div class="setting__row">
          <button class="btn" prop:disabled=move || busy.get()
            on:click=move |_| act::convert(settings, status, busy)>
            "Encrypt earlier uploads"
          </button>
          <button class="btn" prop:disabled=move || busy.get()
            on:click=move |_| act::disable(settings, status, busy)>
            "Turn off encryption"
          </button>
          <button class="btn" prop:disabled=move || busy.get()
            on:click=move |_| act::forget(settings, status)>
            "Forget key here"
          </button>
        </div>
        <p class="setting__hint">
          "\u{201c}Turn off encryption\u{201d} decrypts every document for the \
           whole account and re-uploads it as plaintext, in the background."
        </p>
      })}

      // Account ON + initialized, this browser has no key: prompt to paste it.
      {move || (enabled() && !has_key() && initialized()).then(|| view! {
        <p class="setting__hint">
          "This account uses encryption, but this browser doesn\u{2019}t have \
           the key yet. Paste it from your password manager to read and sync \
           encrypted documents here."
        </p>
        <input class="setting__text" type="password" placeholder="Account key"
          autocomplete="off" autocapitalize="off" spellcheck="false"
          prop:value=move || key_input.get()
          on:input=move |ev| key_input.set(event_target_value(&ev))/>
        <button class="btn btn--primary" prop:disabled=move || busy.get()
          on:click=move |_| act::adopt_key(settings, key_input, status, busy)>
          "Use this key"
        </button>
      })}

      {move || {
        let s = status.get();
        (!s.is_empty()).then(|| view! { <p class="setting__hint">{s}</p> })
      }}
    </section>
  }
}
