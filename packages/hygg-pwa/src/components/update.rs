//! "Update available" banner. Polls the Pages version manifest
//! (`versions.json` at the site root) for a newer build than the one running
//! and offers a one-tap upgrade:
//!
//! - a rolling deploy — the shared `/hygg/` root or the `/hygg/main/` channel —
//!   upgrades by reloading, where the network-first service worker picks up the
//!   fresh HTML shell and its new content-hashed assets;
//! - a pinned `/hygg/<tag>/` deploy is frozen, so a reload would stay put —
//!   there, upgrading hops to the site root instead.
//!
//! It is styled like the install banner (reusing the `.install` classes) and,
//! like it, dismissible — but a dismissal is remembered *per version*, so a
//! fresh release re-prompts rather than staying silent forever.
//!
//! Inert wherever there's nothing to update to: the Tauri shell (native, store
//! updated) and a dev server / any origin served from "/" (no sibling
//! manifest). Best-effort throughout — offline or a missing manifest simply
//! leaves the banner hidden.

use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::build_info as bi;

/// The newest version the user has explicitly dismissed. Kept so we don't
/// re-nag for it while still re-prompting the moment something newer ships.
const DISMISS_KEY: &str = "hygg.update_dismissed";

/// Poll cadence while the app stays open.
const CHECK_INTERVAL_MS: u32 = 60 * 60 * 1000;
/// Floor on how often a focus/visibility wake may re-poll, so returning to a
/// long-open tab checks promptly without a check per flicker.
const MIN_RECHECK_MS: f64 = 10.0 * 60.0 * 1000.0;

/// The one field of `versions.json` we read (the manifest also lists every
/// pinned version, which we ignore).
#[derive(Deserialize)]
struct Manifest {
  latest: Option<String>,
}

/// Where this build is deployed, and how "upgrade" should behave there.
#[derive(Clone)]
struct Deploy {
  /// Absolute URL of the version manifest at the Pages site root.
  manifest_url: String,
  /// The site root ("https://host/hygg/"), where the rolling latest lives.
  site_root: String,
  /// A pinned `/hygg/<tag>/` deploy is frozen — upgrading means leaving it for
  /// the site root, not reloading in place.
  pinned: bool,
}

impl Deploy {
  /// Resolve the current deploy, or `None` when there's nothing to check
  /// against (the Tauri shell, or an origin served from "/").
  fn detect() -> Option<Deploy> {
    // The native shell updates through its app store, never this banner.
    if crate::tauri_ipc::in_tauri() {
      return None;
    }
    let origin = web_sys::window()?.location().origin().ok()?;
    // The deploy base: "/hygg", "/hygg/0.1.26", "/hygg/main", or "" off Pages.
    let base = crate::app::deploy_base();
    let mut segments = base.trim_start_matches('/').split('/');
    let project = segments.next().unwrap_or("");
    if project.is_empty() {
      // A dev server or an origin root, with no sibling version manifest.
      return None;
    }
    // The Pages site root: origin + the project segment, the same for the root
    // deploy, a pinned one, and the main channel. `versions.json` lives there.
    let site_root = format!("{origin}/{project}/");
    // The segment past the project root names a pinned deploy when it's a bare
    // version; the root (no further segment) and `main` are rolling, and so
    // upgrade by reloading in place.
    let pinned = segments.next().map(is_version).unwrap_or(false);
    Some(Deploy {
      manifest_url: format!("{site_root}versions.json"),
      site_root,
      pinned,
    })
  }

  /// Apply the update: reload a rolling deploy in place, or hop a pinned one to
  /// the rolling site root.
  fn apply(&self) {
    let Some(loc) = web_sys::window().map(|w| w.location()) else {
      return;
    };
    if self.pinned {
      let _ = loc.assign(&self.site_root);
    } else {
      let _ = loc.reload();
    }
  }
}

#[component]
pub fn UpdatePrompt() -> impl IntoView {
  // `Some(version)` once a strictly-newer, non-dismissed build is available.
  let available = RwSignal::new(Option::<String>::None);
  // Yield the bottom corner to the install banner while it's up (both are
  // fixed-position, so they'd otherwise overlap).
  let install_visible =
    use_context::<crate::components::InstallVisible>().map(|v| v.0);

  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    let Some(deploy) = Deploy::detect() else {
      return;
    };

    // Raw wall clock (not the skew-corrected one) — this only throttles local
    // re-checks, never orders anything across devices.
    let last_check = StoredValue::new(0.0_f64);
    let check = move || {
      last_check.set_value(crate::clock::local_ms());
      let url = deploy.manifest_url.clone();
      spawn_local(async move {
        if let Some(latest) = fetch_latest(&url).await
          && is_newer(&latest, bi::VERSION)
          && dismissed_version().as_deref() != Some(latest.as_str())
        {
          available.set(Some(latest));
        }
      });
    };

    // Check now, then on a slow interval for the life of the app.
    check();
    Interval::new(CHECK_INTERVAL_MS, check.clone()).forget();

    // …and re-check (throttled) when the user returns to a long-open tab,
    // which is exactly when an installed, never-reloaded PWA has fallen behind.
    if let Some(win) = web_sys::window() {
      let wake = move || {
        if crate::clock::local_ms() - last_check.get_value() >= MIN_RECHECK_MS {
          check();
        }
      };
      let cb = Closure::<dyn FnMut()>::new(wake);
      let _ = win
        .add_event_listener_with_callback("focus", cb.as_ref().unchecked_ref());
      if let Some(doc) = win.document() {
        let _ = doc.add_event_listener_with_callback(
          "visibilitychange",
          cb.as_ref().unchecked_ref(),
        );
      }
      cb.forget();
    }
  });

  let dismiss = move |_| {
    if let Some(v) = available.get_untracked() {
      set_dismissed_version(&v);
    }
    available.set(None);
  };
  let upgrade = move |_| {
    if let Some(deploy) = Deploy::detect() {
      deploy.apply();
    }
  };

  view! {
    {move || match available.get() {
      Some(latest) if !install_visible.map(|v| v.get()).unwrap_or(false) =>
        view! {
        <div class="install">
          <div class="install__text">
            <strong>"Update available"</strong>
            <span>{format!("hygg {} \u{2192} {}", bi::VERSION, latest)}</span>
          </div>
          <div class="install__actions">
            <button class="btn btn--primary" on:click=upgrade>"Update"</button>
            <button class="install__dismiss" on:click=dismiss
              aria-label="Dismiss">"\u{00d7}"</button>
          </div>
        </div>
      }.into_any(),
      _ => ().into_any(),
    }}
  }
}

/// Fetch the manifest's `latest`, or `None` on any failure (offline, 404, bad
/// JSON). Cache-busted with a query string: `versions.json` is a mutable
/// pointer, and both the HTTP cache and — on the root deploy — the service
/// worker would otherwise hand back a stale copy that never reflects a deploy.
async fn fetch_latest(url: &str) -> Option<String> {
  let bust = crate::clock::local_ms() as u64;
  let resp = Request::get(&format!("{url}?_={bust}"))
    .header("Accept", "application/json")
    .header("Cache-Control", "no-cache")
    .send()
    .await
    .ok()?;
  if !resp.ok() {
    return None;
  }
  resp.json::<Manifest>().await.ok()?.latest
}

/// Is `latest` a strictly newer version than `current`? Conservative: anything
/// that doesn't parse as `X.Y.Z` on both sides is treated as "not newer", so a
/// malformed manifest never nags.
fn is_newer(latest: &str, current: &str) -> bool {
  match (parse_version(latest), parse_version(current)) {
    (Some(l), Some(c)) => l > c,
    _ => false,
  }
}

/// Whether a path segment looks like a version (so it names a pinned deploy).
fn is_version(s: &str) -> bool {
  parse_version(s).is_some()
}

/// Parse a bare or `v`-prefixed `X.Y.Z[-pre]` into an orderable key, or `None`
/// when it isn't a version. A release sorts above its own pre-releases (the
/// `true` outranks `false` at an equal `X.Y.Z`), matching the ordering the
/// manifest generator uses.
fn parse_version(s: &str) -> Option<(u32, u32, u32, bool, String)> {
  let s = s.trim().strip_prefix('v').unwrap_or(s.trim());
  let (core, pre) = match s.find(['-', '+']) {
    Some(i) => (&s[..i], s[i + 1..].to_string()),
    None => (s, String::new()),
  };
  let mut nums = core.split('.');
  let major = nums.next()?.parse().ok()?;
  let minor = nums.next()?.parse().ok()?;
  let patch = nums.next()?.parse().ok()?;
  if nums.next().is_some() {
    return None;
  }
  Some((major, minor, patch, pre.is_empty(), pre))
}

fn storage() -> Option<web_sys::Storage> {
  web_sys::window()?.local_storage().ok().flatten()
}

fn dismissed_version() -> Option<String> {
  storage()?.get_item(DISMISS_KEY).ok().flatten()
}

fn set_dismissed_version(v: &str) {
  if let Some(s) = storage() {
    let _ = s.set_item(DISMISS_KEY, v);
  }
}
