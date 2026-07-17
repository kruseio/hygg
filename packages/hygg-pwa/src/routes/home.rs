//! Home: a server-style reading dashboard — summary stats over a library grid
//! where each book shows its progress, percentage, and when it was last read —
//! plus file import. Refreshing auto-syncs from the server when connected;
//! everything otherwise lives offline in IndexedDB.

use std::collections::HashMap;

use gloo_file::File as GlooFile;
use gloo_file::futures::read_as_bytes;
use hygg_shared::sync::SyncMode;
use hygg_shared::sync::proto::DenialBody;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use super::home_menu::{card_menu_modal, dots_icon, refresh_icon};
use super::home_support::{
  download_bodies, fmt_duration, fmt_relative, load_library_and_progress,
  sync_metadata,
};
use super::import_flow::{ImportResult, do_import, upload_book_if_synced};
use crate::app::{SettingsCtx, link};
use crate::components::TopBar;
use crate::model::{BookSummary, Progress};

type ProgressMap = HashMap<String, Progress>;

#[component]
pub fn Home() -> impl IntoView {
  let settings = expect_context::<SettingsCtx>();
  let library: RwSignal<Vec<BookSummary>> = RwSignal::new(Vec::new());
  let progress: RwSignal<ProgressMap> = RwSignal::new(HashMap::new());
  let status = RwSignal::new(String::new());
  // The server's own words when it declines to convert an import.
  let denial = RwSignal::new(None::<DenialBody>);
  // The library card whose "more options" sheet is open (`None` = closed).
  let menu = RwSignal::new(None::<String>);
  let file_ref = NodeRef::<leptos::html::Input>::new();

  let reload = move || {
    spawn_local(async move {
      let (lib, prog) = load_library_and_progress().await;
      library.set(lib);
      progress.set(prog);
    });
  };
  reload();

  // Sync from the server in two phases so the UI never waits on a download:
  // first pull library metadata + reading positions and render them, then fetch
  // document bytes in the background so a tapped document is already
  // downloaded. `force` runs even with background auto-sync off (the explicit
  // "sync now").
  let do_sync = move |_force: bool| {
    // Background and "Sync now" share master-gated credentials; the scope gates
    // which documents actually push, per-document.
    let Some(creds) = sync_creds(&settings) else {
      return;
    };
    let col = settings.read().import_col;
    spawn_local(async move {
      let pending = sync_metadata(&creds).await;
      let (lib, prog) = load_library_and_progress().await;
      library.set(lib);
      progress.set(prog);
      if !pending.is_empty() {
        status.set(format!(
          "Downloading {} document(s) in the background\u{2026}",
          pending.len()
        ));
        let done = download_bodies(&creds, col, pending).await;
        status.set(if done > 0 {
          format!("Synced {done} document(s) from server.")
        } else {
          String::new()
        });
      }
    });
  };

  // On open, sync once; then subscribe to the server's live `changed` stream so
  // a peer's changes refresh the library the moment they happen. The browser
  // reconnects the stream on its own, so there's no periodic poll to fall back
  // on; the subscription is closed when the home unmounts.
  Effect::new(move |prev: Option<()>| {
    if prev.is_some() {
      return;
    }
    do_sync(false);
    if let Some(creds) = sync_creds(&settings) {
      let events = crate::sse::connect(&creds, move || do_sync(false));
      // `SendWrapper` carries the non-`Send` `EventSource` across Leptos's
      // cleanup bound (sound: the app is single-threaded).
      let events = send_wrapper::SendWrapper::new(events);
      on_cleanup(move || drop(events));
    }
  });

  let on_files = move |ev: web_sys::Event| {
    let input = ev.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok());
    let Some(files) = input.and_then(|i| i.files()) else {
      return;
    };
    let col = settings.read().import_col;
    let creds = sync_creds(&settings);
    let scope = settings.with(|s| s.auto_sync_scope);
    denial.set(None);
    for idx in 0..files.length() {
      let Some(file) = files.get(idx) else { continue };
      let gfile = GlooFile::from(file);
      let name = gfile.name();
      let creds = creds.clone();
      status.set(format!("Importing {name}\u{2026}"));
      spawn_local(async move {
        let bytes = match read_as_bytes(&gfile).await {
          Ok(b) => b,
          Err(e) => {
            status.set(format!("Read failed: {e}"));
            return;
          }
        };
        match do_import(name, bytes, col, creds, scope).await {
          ImportResult::Saved => {
            status.set(String::new());
            reload();
          }
          ImportResult::Message(m) => status.set(m),
          ImportResult::Denied(body) => {
            status.set(body.error.clone());
            denial.set(Some(body));
          }
        }
      });
    }
  };

  let delete = move |id: String| {
    spawn_local(async move {
      let _ = crate::storage::delete_book(&id).await;
      reload();
    });
  };

  // Change this device's local sync preference for a document (`None` =
  // inherit the account-wide ceiling). Purely local; the effective mode is the
  // more restrictive of this and the server ceiling.
  let set_sync = move |id: String, mode: Option<SyncMode>| {
    spawn_local(async move {
      let _ = crate::storage::set_local_sync_mode(&id, mode).await;
      reload();
    });
  };

  // Add or remove a document from auto-sync. Opting in uploads it now (bytes or
  // metadata per its sync mode) so it reaches other devices, not just its
  // future progress.
  let set_optin = move |id: String, on: bool| {
    spawn_local(async move {
      let _ = crate::storage::set_auto_sync_optin(&id, on).await;
      if on && let Some(creds) = settings.with(|s| s.sync_creds()) {
        let scope = settings.with(|s| s.auto_sync_scope);
        upload_book_if_synced(&creds, &id, scope).await;
      }
      reload();
    });
  };

  view! {
    <TopBar title=Signal::derive(String::new) visible=Signal::derive(|| true)/>
    <main class="home">
      {move || stats_view(&library.get(), &progress.get())}

      // Import and "sync now" are the two ways documents enter this library,
      // so they sit together rather than a bar apart.
      <div class="home__import">
        <input type="file" multiple node_ref=file_ref class="hidden"
          accept=".txt,.text,.md,.markdown,.epub,.pdf,.docx,.doc,.odt,.rtf"
          on:change=on_files/>
        <button class="btn btn--primary" on:click=move |_| {
          if let Some(i) = file_ref.get() { i.click(); }
        }>"Import document"</button>
        <button class="iconbtn iconbtn--ring" aria-label="Sync now"
          title="Sync now" on:click=move |_| do_sync(true)>{refresh_icon()}</button>
        {move || {
          let s = status.get();
          (!s.is_empty()).then(|| view! { <span class="home__status">{s}</span> })
        }}
      </div>

      // Every word here is the server's: the banner only renders when the
      // refusal came with somewhere to go.
      {move || denial.get().and_then(|d| {
        let url = d.action_url.clone()?;
        let label = d.action_label.clone()?;
        Some(view! {
          <a class="server-notice" href=url target="_blank" rel="noopener">
            <strong>{label}</strong>
            <span>{d.error.clone()}</span>
          </a>
        })
      })}

      {move || {
        let books = library.get();
        if books.is_empty() {
          return view! {
            <p class="home__empty">
              "Your library is empty. Import a PDF, EPUB, or text file to start reading."
            </p>
          }.into_any();
        }
        view! {
          <ul class="library">
            <For each=move || library.get() key=|b| b.id.clone()
              children=move |b: BookSummary| {
                let p = progress.with(|m| m.get(&b.id).copied().unwrap_or_default());
                book_card(b, p, menu)
              }
            />
          </ul>
        }.into_any()
      }}

      // The open card's "more options" sheet, overlaid on the whole home.
      {move || menu.get()
        .and_then(|id| library.with(|lib| lib.iter().find(|b| b.id == id).cloned()))
        .map(|b| card_menu_modal(b, menu, delete, set_sync, set_optin))}
    </main>
  }
}

/// The summary stat row mirroring the server home: total reading time, document
/// count, how many were started, and how many finished.
fn stats_view(
  library: &[BookSummary],
  progress: &ProgressMap,
) -> impl IntoView + use<> {
  let seconds: f64 = progress.values().map(|p| p.seconds).sum();
  let started = progress.values().filter(|p| p.started()).count();
  let finished = progress.values().filter(|p| p.finished()).count();
  let documents = library.len();
  view! {
    <section class="stats">
      <div class="stat"><strong>{fmt_duration(seconds)}</strong><span>"reading time"</span></div>
      <div class="stat"><strong>{documents}</strong><span>"documents"</span></div>
      <div class="stat"><strong>{started}</strong><span>"started"</span></div>
      <div class="stat"><strong>{finished}</strong><span>"finished"</span></div>
    </section>
  }
}

/// One library card: title, a progress bar with percentage, format, and a
/// "last read" + total reading-time line — a touch-sized version of the
/// server's book cards. The
/// sync + remove controls live behind the "more options" sheet (see
/// [`card_menu_modal`]); the dots button opens it for this card.
fn book_card(
  b: BookSummary,
  p: Progress,
  menu: RwSignal<Option<String>>,
) -> impl IntoView {
  let id = b.id.clone();
  let menu_id = b.id.clone();
  let pct = p.percent.round() as i64;
  let last = fmt_relative(p.updated_at);
  // Total time spent reading this document (blank until it's been opened).
  let read = if p.seconds >= 1.0 {
    format!(" \u{00b7} {} read", fmt_duration(p.seconds))
  } else {
    String::new()
  };
  let sub = match (p.started(), last.is_empty()) {
    (true, false) => format!("Last read {last}{read}"),
    (true, true) => format!("In progress{read}"),
    _ => "Not started".to_string(),
  };
  view! {
    <li class="card">
      <A href=link(&format!("/read/{}", id)) attr:class="card__open">
        <span class="card__title">{b.title.clone()}</span>
        <div class="card__bar"><span style=format!("width:{pct}%")></span></div>
        <span class="card__meta">
          {format!("{pct}%")} " \u{00b7} " {b.format.to_uppercase()}
        </span>
        <span class="card__sub">{sub}</span>
      </A>
      <button class="card__menu" aria-label="More options"
        on:click=move |_| menu.set(Some(menu_id.clone()))>{dots_icon()}</button>
    </li>
  }
}

/// Master-gated sync credentials: `None` when the master switch is off
/// (serverless) or not connected.
fn sync_creds(settings: &SettingsCtx) -> Option<crate::sync::Creds> {
  settings.with(|s| s.sync_creds())
}
