//! The library card's "more options" sheet — a modal holding the per-device
//! sync selector and the remove control moved off the card face — plus the
//! small inline icons the home uses (sync, more, close). Split out of
//! `home.rs`.

use hygg_shared::sync::SyncMode;
use leptos::ev;
use leptos::prelude::*;

use crate::model::BookSummary;

/// The card menu modal for `b`: a sync selector and an X remove button.
/// Dismissed by the backdrop, the close control, or Escape; removing prompts a
/// confirmation sheet ([`remove_confirm_modal`]) before anything is deleted.
pub(super) fn card_menu_modal(
  b: BookSummary,
  menu: RwSignal<Option<String>>,
  // `delete` flows into the confirmation's reactive render closure, which
  // Leptos requires to be `Send`.
  delete: impl Fn(String) + Copy + Send + 'static,
  set_sync: impl Fn(String, Option<SyncMode>) + Copy + 'static,
  set_optin: impl Fn(String, bool) + Copy + 'static,
) -> impl IntoView {
  let sync_id = b.id.clone();
  let optin_id = b.id.clone();
  let optin_checked = b.auto_sync_optin;
  // Whether the remove-confirmation sheet is showing over the options sheet.
  let confirm = RwSignal::new(false);
  // Escape peels one layer at a time: the confirmation first, then the sheet.
  let handle = window_event_listener(ev::keydown, move |ev| {
    if ev.key() == "Escape" {
      if confirm.get_untracked() {
        confirm.set(false);
      } else {
        menu.set(None);
      }
    }
  });
  on_cleanup(move || handle.remove());
  // The local preference drives the selector (`None` shows "inherit"); when the
  // account ceiling clamps this device, note the effective mode below it.
  let selected = b
    .local_sync_mode
    .map(|m| m.to_string())
    .unwrap_or_else(|| "inherit".to_string());
  let capped =
    b.effective_sync_mode() != b.local_sync_mode.unwrap_or(SyncMode::Full);
  let note = capped
    .then(|| format!("Synced as {} on this device.", b.effective_sync_mode()));
  let title = b.title.clone();
  view! {
    <div class="modal" on:click=move |_| menu.set(None)>
      <div class="modal__card" on:click=|ev| ev.stop_propagation()>
        <div class="modal__head">
          <span class="modal__title">{b.title.clone()}</span>
          <button class="iconbtn" aria-label="Close"
            on:click=move |_| menu.set(None)>{x_icon()}</button>
        </div>
        <label class="modal__label" for="modal-sync">"Sync on this device"</label>
        <select id="modal-sync" class="modal__sync" prop:value=selected
          on:change=move |ev| {
            let v = event_target_value(&ev);
            let mode = if v == "inherit" { None } else { v.parse::<SyncMode>().ok() };
            set_sync(sync_id.clone(), mode);
          }>
          <option value="inherit">"Sync: inherit"</option>
          <option value="full">"Sync: full"</option>
          <option value="metadata">"Sync: metadata"</option>
          <option value="off">"Sync: off"</option>
        </select>
        {note.map(|n| view! { <p class="modal__note">{n}</p> })}
        <label class="toggle">
          <input type="checkbox" prop:checked=optin_checked
            on:change=move |ev| {
              let on = event_target_checked(&ev);
              set_optin(optin_id.clone(), on);
            }/>
          <span>"Auto-sync this document"</span>
        </label>
        <div class="modal__row">
          <span>"Remove from library"</span>
          <button class="modal__del" aria-label="Remove"
            on:click=move |_| confirm.set(true)>
            {x_icon()}
          </button>
        </div>
      </div>
    </div>
    {move || confirm.get()
      .then(|| remove_confirm_modal(b.id.clone(), title.clone(), menu, confirm, delete))}
  }
}

/// The remove confirmation, layered above the options sheet. Cancel, the close
/// control, the backdrop, and Escape all back out to the sheet; "Remove"
/// deletes `id` and closes everything.
fn remove_confirm_modal(
  id: String,
  title: String,
  menu: RwSignal<Option<String>>,
  confirm: RwSignal<bool>,
  delete: impl Fn(String) + Copy + Send + 'static,
) -> impl IntoView {
  view! {
    <div class="modal modal--confirm" on:click=move |_| confirm.set(false)>
      <div class="modal__card" on:click=|ev| ev.stop_propagation()>
        <div class="modal__head">
          <span class="modal__title">"Remove document"</span>
          <button class="iconbtn" aria-label="Close"
            on:click=move |_| confirm.set(false)>{x_icon()}</button>
        </div>
        <p class="modal__text">
          "Remove \u{201c}"{title}"\u{201d} from this device? Its downloaded copy "
          "and reading progress here are cleared; synced documents can be "
          "re-downloaded later."
        </p>
        <div class="modal__actions">
          <button class="btn" on:click=move |_| confirm.set(false)>"Cancel"</button>
          <button class="btn btn--danger" on:click=move |_| {
            delete(id.clone());
            menu.set(None);
          }>"Remove"</button>
        </div>
      </div>
    </div>
  }
}

/// Sync now — a clockwise "refresh" pair of arrows (home top bar).
pub(super) fn refresh_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="22" height="22" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <polyline points="23 4 23 10 17 10"/>
      <polyline points="1 20 1 14 7 14"/>
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
    </svg>
  }
}

/// The card's "more options" affordance — three dots opening its menu.
pub(super) fn dots_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor"
      stroke="none">
      <circle cx="5" cy="12" r="2"/>
      <circle cx="12" cy="12" r="2"/>
      <circle cx="19" cy="12" r="2"/>
    </svg>
  }
}

/// Dismiss / remove — a plain X (the modal's close and remove controls).
pub(super) fn x_icon() -> impl IntoView {
  view! {
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none"
      stroke="currentColor" stroke-width="2" stroke-linecap="round"
      stroke-linejoin="round">
      <line x1="18" y1="6" x2="6" y2="18"/>
      <line x1="6" y1="6" x2="18" y2="18"/>
    </svg>
  }
}
