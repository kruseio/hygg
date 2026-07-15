//! Reader settings (font size, theme, server URL…) persisted to `localStorage`.
//! Small and synchronous — these are user preferences, not document data.

use serde::{Deserialize, Serialize};

const KEY: &str = "hygg.settings";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Theme {
  Dark,
  Light,
  Sepia,
}

impl Theme {
  /// CSS class applied to `<html>` so the stylesheet can theme everything.
  pub fn css_class(self) -> &'static str {
    match self {
      Theme::Dark => "theme-dark",
      Theme::Light => "theme-light",
      Theme::Sepia => "theme-sepia",
    }
  }
}

/// How the reader renders a document's figures and tables. A pure view choice:
/// it never changes the flattened line/anchor model, so switching it (or using
/// a different mode than another device) never moves a reading position.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ImageMode {
  /// Hide figures/tables (blank space where they'd be), for a text-only read.
  None,
  /// Colored ASCII-art half-blocks — exactly what the terminal / CLI show.
  Ascii,
  /// Crisp raster images and rasterized tables (the default).
  Images,
}

impl ImageMode {
  pub const ALL: [ImageMode; 3] =
    [ImageMode::None, ImageMode::Ascii, ImageMode::Images];

  pub fn label(self) -> &'static str {
    match self {
      ImageMode::None => "None",
      ImageMode::Ascii => "ASCII",
      ImageMode::Images => "Images",
    }
  }
}

fn default_image_mode() -> ImageMode {
  ImageMode::Images
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
  /// Text zoom multiplier on top of the width-fitted base size (1.0 = fill the
  /// screen). The reader auto-sizes the monospace column to the viewport.
  #[serde(default = "default_zoom")]
  pub text_zoom: f32,
  pub theme: Theme,
  /// How figures and tables are rendered in the reader.
  #[serde(default = "default_image_mode")]
  pub image_mode: ImageMode,
  /// Justification width used when importing new documents.
  pub import_col: usize,
  /// The server this build points at out of the box.
  pub server_url: String,
  /// Account username (email), sent with the token on every request. The
  /// server rejects a token whose owner's email doesn't match, so both are
  /// needed. `None` = offline-only.
  #[serde(default)]
  pub username: Option<String>,
  /// Device bearer token, set once connected (`None` = offline-only).
  #[serde(default)]
  pub api_token: Option<String>,
  /// Server-assigned device id for this browser.
  #[serde(default)]
  pub device_id: Option<String>,
  /// A stable per-browser machine id. The device token binds to it on first
  /// use and the server refuses the token from any other machine, so one token
  /// can't be copied between browsers. Generated once, then persisted here.
  #[serde(default)]
  pub machine_id: Option<String>,
  /// Master sync switch. `false` = fully serverless (no background sync, no
  /// "Sync now"); overrides the auto-sync scope entirely.
  #[serde(default = "default_true")]
  pub sync_enabled: bool,
  /// Automatic-sync scope: which documents sync without being touched. `books`
  /// (the default) syncs book-like documents; `all` syncs everything; `manual`
  /// syncs only per-document opt-ins.
  #[serde(default)]
  pub auto_sync_scope: hygg_shared::sync::AutoSyncPolicy,
  /// Legacy `auto_sync` boolean, read once on load for migration then dropped.
  #[serde(default, rename = "auto_sync", skip_serializing)]
  legacy_auto_sync: Option<bool>,
  /// Text-to-speech rate (0.5–2.0; 1.0 = normal).
  #[serde(default = "default_rate")]
  pub tts_rate: f32,
}

fn default_true() -> bool {
  true
}

fn default_rate() -> f32 {
  1.0
}

fn default_zoom() -> f32 {
  1.0
}

impl Settings {
  /// Connected to a server (a device token is present)?
  pub fn is_connected(&self) -> bool {
    self.api_token.is_some()
  }

  /// Ensure a stable per-browser machine id exists (generating one on first
  /// use) and return it. The caller is responsible for persisting.
  pub fn ensure_machine_id(&mut self) -> String {
    if self.machine_id.as_deref().unwrap_or_default().is_empty() {
      self.machine_id = Some(uuid::Uuid::new_v4().to_string());
    }
    self.machine_id.clone().unwrap_or_default()
  }

  /// The full request credentials, or `None` when not fully connected (a token,
  /// username and machine id are all required now). Not gated by the master
  /// switch — used for connection setup/validation (`/me`).
  pub fn creds(&self) -> Option<crate::sync::Creds> {
    Some(crate::sync::Creds {
      server: self.server_url.clone(),
      token: self.api_token.clone().filter(|s| !s.is_empty())?,
      username: self.username.clone().filter(|s| !s.is_empty())?,
      machine_id: self.machine_id.clone().filter(|s| !s.is_empty())?,
    })
  }

  /// Master-gated credentials for any *data sync* path (background or "Sync
  /// now"). `None` when the master switch is off (serverless) or not connected.
  /// The auto-sync *scope* gates which documents actually sync, per-document.
  pub fn sync_creds(&self) -> Option<crate::sync::Creds> {
    if !self.sync_enabled {
      return None;
    }
    self.creds()
  }
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      text_zoom: 1.0,
      theme: Theme::Dark,
      image_mode: default_image_mode(),
      import_col: 64,
      // The default server for this build. A local / self-host dev build can
      // point at its own server by setting `HYGG_PWA_SERVER_URL` (build.rs
      // bakes it in from `packages/hygg-pwa/.env`, which is gitignored) — so no
      // address is hardcoded (or committed) here.
      server_url: option_env!("HYGG_PWA_SERVER_URL")
        .unwrap_or("https://hygg.kruseio.com")
        .to_string(),
      username: None,
      api_token: None,
      device_id: None,
      machine_id: None,
      sync_enabled: true,
      auto_sync_scope: hygg_shared::sync::AutoSyncPolicy::Books,
      legacy_auto_sync: None,
      tts_rate: 1.0,
    }
  }
}

impl Settings {
  /// Load from `localStorage`, falling back to defaults on any error so the app
  /// always boots.
  pub fn load() -> Self {
    let mut s: Settings = local_storage()
      .and_then(|s| s.get_item(KEY).ok().flatten())
      .and_then(|raw| serde_json::from_str(&raw).ok())
      .unwrap_or_default();
    // A browser that had explicitly turned auto-sync off becomes opt-in-only
    // (nothing auto-syncs, but the connection stays usable — respecting the
    // opt-out without a surprise re-upload); everything else adopts the new
    // book-only default via the `auto_sync_scope` serde default.
    if s.legacy_auto_sync == Some(false) {
      s.auto_sync_scope = hygg_shared::sync::AutoSyncPolicy::Manual;
    }
    s.legacy_auto_sync = None;
    s
  }

  /// Best-effort persist; ignores quota/availability errors.
  pub fn save(&self) {
    if let (Some(s), Ok(raw)) = (local_storage(), serde_json::to_string(self)) {
      let _ = s.set_item(KEY, &raw);
    }
  }
}

fn local_storage() -> Option<web_sys::Storage> {
  web_sys::window()?.local_storage().ok().flatten()
}
