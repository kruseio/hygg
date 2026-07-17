//! Install prompt banner. Shows once, is dismissible, and never reappears after
//! a dismissal (persisted to localStorage).
//!
//! - Chromium/Android: captures `beforeinstallprompt` (via the index.html shim)
//!   and offers a real "Install" button.
//! - iOS/iPadOS: Safari has no programmatic install, so when not already
//!   running standalone we show the "Add to Home Screen" instructions instead.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

const DISMISS_KEY: &str = "hygg.install_dismissed";

/// Shared flag: is the install banner currently on screen? Provided by `App`
/// and mirrored here from [`Mode`]. The update banner reads it to stay hidden
/// while this one is up, so the two fixed bottom banners never stack on top of
/// each other — they can co-occur only in a browser tab that is both installable
/// and behind, and there the install CTA takes the corner (a tab picks up a
/// fresh build on any navigation regardless, via the network-first worker).
#[derive(Clone, Copy)]
pub struct InstallVisible(pub RwSignal<bool>);

#[derive(Clone, Copy, PartialEq)]
enum Mode {
  Hidden,
  Install,
  Ios,
}

#[component]
pub fn InstallPrompt() -> impl IntoView {
  let mode = RwSignal::new(Mode::Hidden);

  // Keep the shared visibility flag in step with our mode, so a sibling banner
  // can defer to us. No-op when the context isn't provided (e.g. in isolation).
  if let Some(vis) = use_context::<InstallVisible>() {
    Effect::new(move |_| vis.0.set(mode.get() != Mode::Hidden));
  }

  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    // Inside the Tauri shell the app is already "installed" natively — there's
    // no browser install prompt and no "Add to Home Screen", so never offer it.
    if crate::tauri_ipc::in_tauri() {
      return;
    }
    if dismissed() || is_standalone() {
      return;
    }
    if deferred_prompt_available() {
      mode.set(Mode::Install);
    } else if is_ios() {
      mode.set(Mode::Ios);
    }
    // A `beforeinstallprompt` can also arrive after mount.
    if let Some(win) = web_sys::window() {
      let cb = Closure::<dyn FnMut()>::new(move || {
        if !dismissed() {
          mode.set(Mode::Install);
        }
      });
      let _ = win.add_event_listener_with_callback(
        "hygg:installable",
        cb.as_ref().unchecked_ref(),
      );
      cb.forget();
    }
  });

  let dismiss = move |_| {
    set_dismissed();
    mode.set(Mode::Hidden);
  };
  let install = move |_| {
    call_install();
    set_dismissed();
    mode.set(Mode::Hidden);
  };

  view! {
    {move || match mode.get() {
      Mode::Hidden => ().into_any(),
      Mode::Install => view! {
        <div class="install">
          <div class="install__text">
            <strong>"Install hygg"</strong>
            <span>"Add it to your home screen for full-screen, offline reading."</span>
          </div>
          <div class="install__actions">
            <button class="btn btn--primary" on:click=install>"Install"</button>
            <button class="install__dismiss" on:click=dismiss
              aria-label="Dismiss">"×"</button>
          </div>
        </div>
      }.into_any(),
      Mode::Ios => view! {
        <div class="install">
          <div class="install__text">
            <strong>"Install hygg"</strong>
            <span>"Tap Share, then \u{201c}Add to Home Screen\u{201d} to read offline."</span>
          </div>
          <button class="install__dismiss" on:click=dismiss
            aria-label="Dismiss">"×"</button>
        </div>
      }.into_any(),
    }}
  }
}

fn storage() -> Option<web_sys::Storage> {
  web_sys::window()?.local_storage().ok().flatten()
}

fn dismissed() -> bool {
  storage().and_then(|s| s.get_item(DISMISS_KEY).ok().flatten()).is_some()
}

fn set_dismissed() {
  if let Some(s) = storage() {
    let _ = s.set_item(DISMISS_KEY, "1");
  }
}

/// Already launched as an installed app? Then there's nothing to prompt.
fn is_standalone() -> bool {
  let Some(win) = web_sys::window() else {
    return false;
  };
  if let Ok(Some(mq)) = win.match_media("(display-mode: standalone)")
    && mq.matches()
  {
    return true;
  }
  // iOS exposes navigator.standalone (non-standard; read via reflection).
  js_sys::Reflect::get(&win.navigator(), &JsValue::from_str("standalone"))
    .map(|v| v.is_truthy())
    .unwrap_or(false)
}

fn is_ios() -> bool {
  web_sys::window()
    .map(|w| w.navigator().user_agent().unwrap_or_default())
    .map(|ua| {
      ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod")
    })
    .unwrap_or(false)
}

fn deferred_prompt_available() -> bool {
  web_sys::window()
    .and_then(|w| {
      js_sys::Reflect::get(&w, &JsValue::from_str("__hyggDeferredPrompt")).ok()
    })
    .map(|v| !v.is_null() && !v.is_undefined())
    .unwrap_or(false)
}

fn call_install() {
  if let Some(win) = web_sys::window()
    && let Ok(f) =
      js_sys::Reflect::get(&win, &JsValue::from_str("__hyggInstall"))
    && let Ok(func) = f.dyn_into::<js_sys::Function>()
  {
    let _ = func.call0(&win);
  }
}
