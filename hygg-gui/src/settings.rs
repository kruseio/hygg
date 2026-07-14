//! Reader settings (font size, theme, column width, server URL). Persisted to a
//! config file in the per-user config directory. Small and synchronous — these
//! are user preferences, not document data.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Theme {
  Dark,
  Light,
  Sepia,
}

impl Theme {
  pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Light, Theme::Sepia];

  pub fn label(self) -> &'static str {
    match self {
      Theme::Dark => "Dark",
      Theme::Light => "Light",
      Theme::Sepia => "Sepia",
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
  /// Colored ASCII-art half-blocks — exactly what the terminal / PWA show.
  Ascii,
  /// Crisp raster images and rasterized tables (this GUI's default).
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
  /// window). The reader auto-sizes the monospace column to the viewport.
  #[serde(default = "default_zoom")]
  pub text_zoom: f32,
  pub theme: Theme,
  /// How figures and tables are rendered in the reader.
  #[serde(default = "default_image_mode")]
  pub image_mode: ImageMode,
  /// Justification width used when importing new documents.
  pub import_col: usize,
  /// Optional sync server; the reader works fully offline without it.
  pub server_url: String,
  #[serde(default)]
  pub username: Option<String>,
  #[serde(default)]
  pub api_token: Option<String>,
  /// Stable per-install id the device token is bound to on the server.
  #[serde(default)]
  pub machine_id: Option<String>,
  /// Server-assigned device id for this install (tags pushed sync ops).
  #[serde(default)]
  pub device_id: Option<String>,
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
}

fn default_true() -> bool {
  true
}

fn default_zoom() -> f32 {
  1.0
}

impl Settings {
  /// Connected to a server (a device token is present)?
  pub fn is_connected(&self) -> bool {
    self.api_token.as_deref().is_some_and(|t| !t.is_empty())
  }

  /// Ensure a stable per-install machine id exists (the device token binds to
  /// it on the server), creating one on first connect. Returns the id. The
  /// caller persists the settings afterwards.
  pub fn ensure_machine_id(&mut self) -> String {
    if self.machine_id.as_deref().unwrap_or_default().is_empty() {
      self.machine_id = Some(uuid::Uuid::new_v4().to_string());
    }
    self.machine_id.clone().unwrap_or_default()
  }

  /// Full sync credentials: `None` when the master switch is off (serverless)
  /// or the account isn't fully connected. The auto-sync *scope* gates which
  /// documents push, not whether credentials exist, so background and "Sync
  /// now" share the same credentials.
  pub fn creds(&self) -> Option<crate::sync::Creds> {
    self.creds_manual()
  }

  /// Sync credentials for any sync path. `None` when the master switch is off,
  /// or when the account isn't fully connected (a token, username and machine
  /// id are all required).
  pub fn creds_manual(&self) -> Option<crate::sync::Creds> {
    if !self.sync_enabled {
      return None;
    }
    Some(crate::sync::Creds {
      server: self.server_url.clone(),
      token: self.api_token.clone().filter(|s| !s.is_empty())?,
      username: self.username.clone().filter(|s| !s.is_empty())?,
      machine_id: self.machine_id.clone().filter(|s| !s.is_empty())?,
      device_id: self.device_id.clone().unwrap_or_default(),
    })
  }
}

/// Transient state for the Settings → Account connect form. The saved
/// credentials live in [`Settings`]; this only holds the in-progress inputs and
/// the status/plan lines shown while connecting (not persisted).
#[derive(Default)]
pub struct Account {
  /// Username field (email) while typing; cleared once connected.
  pub user: String,
  /// Device-token field while typing; cleared once connected.
  pub token: String,
  /// Result line under the form ("Connecting…", an error, or "Connected.").
  pub status: String,
  /// The connected account's label, if the server sends one. Shown as-is.
  pub label: String,
  /// A connect/validate request is in flight (disables the button).
  pub busy: bool,
}

impl Default for Settings {
  fn default() -> Self {
    Settings {
      text_zoom: 1.0,
      theme: Theme::Dark,
      image_mode: default_image_mode(),
      import_col: 64,
      server_url: option_env!("HYGG_GUI_SERVER_URL")
        .unwrap_or("https://hygg.kruseio.com")
        .to_string(),
      username: None,
      api_token: None,
      machine_id: None,
      device_id: None,
      sync_enabled: true,
      auto_sync_scope: hygg_shared::sync::AutoSyncPolicy::Books,
      legacy_auto_sync: None,
    }
  }
}

impl Settings {
  /// Load persisted settings, falling back to defaults on any error so the app
  /// always boots.
  pub fn load() -> Self {
    let mut s: Settings = load_raw()
      .and_then(|raw| serde_json::from_str(&raw).ok())
      .unwrap_or_default();
    // A device that had explicitly turned auto-sync off becomes opt-in-only
    // (nothing auto-syncs, but the connection stays usable — respecting the
    // opt-out without a surprise re-upload); everything else adopts the new
    // book-only default via the `auto_sync_scope` serde default.
    if s.legacy_auto_sync == Some(false) {
      s.auto_sync_scope = hygg_shared::sync::AutoSyncPolicy::Manual;
    }
    s.legacy_auto_sync = None;
    s
  }

  /// Best-effort persist; ignores I/O / quota errors.
  pub fn save(&self) {
    if let Ok(raw) = serde_json::to_string(self) {
      save_raw(&raw);
    }
  }
}

// ----------------------------------------------------------------- storage ---

fn config_path() -> Option<std::path::PathBuf> {
  let dirs = directories::ProjectDirs::from("com", "kruseio", "hygg-gui")?;
  Some(dirs.config_dir().join("settings.json"))
}

fn load_raw() -> Option<String> {
  std::fs::read_to_string(config_path()?).ok()
}

fn save_raw(raw: &str) {
  if let Some(path) = config_path() {
    if let Some(parent) = path.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, raw);
  }
}
