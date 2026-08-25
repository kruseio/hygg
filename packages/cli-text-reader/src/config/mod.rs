use crate::utils::{
  ensure_config_file_with_defaults, get_hygg_config_file, parse_bool_env_var,
};
use hygg_shared::sync::AutoSyncPolicy;
use std::collections::HashMap;
use std::path::PathBuf;

mod encryption;
mod env_io;
use env_io::write_env_preserving;

pub use encryption::{
  ENCRYPTION_KEY_ENV, EncryptionConfig, load_encryption_config,
  save_encryption_config,
};

#[derive(Default)]
pub struct AppConfig {
  pub enable_tutorial: Option<bool>,
  pub enable_line_highlighter: Option<bool>,
  pub show_cursor: Option<bool>,
  pub show_progress: Option<bool>,
  pub pdf_ocr: Option<bool>,
  pub tts_enabled: Option<bool>,
  pub tutorial_shown: Option<bool>,
}

/// Text-to-speech narration is on by default; `--tts off` (or `ENABLE_TTS`)
/// turns it off.
pub const DEFAULT_TTS_ENABLED: bool = true;

/// Master sync switch (`SYNC`). `false` = fully serverless: no engine, no
/// reconcile, no manual sync. Overrides the auto-sync scope entirely.
pub const DEFAULT_SYNC_ENABLED: bool = true;

fn get_config_env_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_config_file(".env")
}

pub fn config_env_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_config_env_path()
}

fn ensure_config_file() -> Result<(), Box<dyn std::error::Error>> {
  let config_path = get_config_env_path()?;
  ensure_config_file_with_defaults(
    &config_path,
    "ENABLE_TUTORIAL=true\nENABLE_LINE_HIGHLIGHTER=true\nSHOW_CURSOR=true\nSHOW_PROGRESS=true\nPDF_OCR=false\nENABLE_TTS=true\nENABLE_OSC52=true\nTUTORIAL_SHOWN=false\nSERVER_URL=\nUSERNAME=\nAPI_TOKEN=\nSYNC=true\nAUTO_SYNC=books\nDEVICE_ID=\n",
  )
}

pub fn load_config() -> AppConfig {
  let mut config = AppConfig::default();

  if let Ok(config_path) = get_config_env_path()
    && ensure_config_file().is_ok()
  {
    let file_values = dotenvy::from_path_iter(config_path)
      .ok()
      .map(|iter| iter.filter_map(Result::ok).collect::<HashMap<_, _>>())
      .unwrap_or_default();
    config.enable_tutorial = config_bool("ENABLE_TUTORIAL", &file_values);
    config.enable_line_highlighter =
      config_bool("ENABLE_LINE_HIGHLIGHTER", &file_values);
    config.show_cursor = config_bool("SHOW_CURSOR", &file_values);
    config.show_progress = config_bool("SHOW_PROGRESS", &file_values);
    config.pdf_ocr = config_bool("PDF_OCR", &file_values);
    config.tts_enabled = config_bool("ENABLE_TTS", &file_values);
    config.tutorial_shown = config_bool("TUTORIAL_SHOWN", &file_values);
  }

  config
}

fn config_bool(
  key: &str,
  file_values: &HashMap<String, String>,
) -> Option<bool> {
  parse_bool_env_var(key).or_else(|| {
    file_values.get(key).map(|value| value.eq_ignore_ascii_case("true"))
  })
}

fn config_string(
  key: &str,
  file_values: &HashMap<String, String>,
) -> Option<String> {
  std::env::var(key).ok().or_else(|| file_values.get(key).cloned())
}

fn config_f32(key: &str, file_values: &HashMap<String, String>) -> Option<f32> {
  std::env::var(key)
    .ok()
    .and_then(|v| v.parse().ok())
    .or_else(|| file_values.get(key).and_then(|v| v.parse().ok()))
}

/// Kokoro's highest-quality voice, used as the narration default.
pub const DEFAULT_TTS_VOICE: &str = "af_heart";

/// Narration speed that gives Kokoro enough forward motion without sounding
/// rushed.
pub const DEFAULT_TTS_SPEED: f32 = 1.3;

/// Startup narration voice id and speed. Reads `TTS_VOICE` / `TTS_SPEED` from
/// the environment, then `~/.config/hygg/.env` if it exists, falling back to
/// the default voice (`af_heart`) at speed 1.3. These are only the *startup*
/// values; `:voice` and `:speed` change them live while reading.
pub fn tts_settings() -> (String, f32) {
  let file_values = get_config_env_path()
    .ok()
    .and_then(|path| dotenvy::from_path_iter(path).ok())
    .map(|iter| iter.filter_map(Result::ok).collect::<HashMap<_, _>>())
    .unwrap_or_default();
  let voice = config_string("TTS_VOICE", &file_values)
    .unwrap_or_else(|| DEFAULT_TTS_VOICE.to_string());
  let speed =
    config_f32("TTS_SPEED", &file_values).unwrap_or(DEFAULT_TTS_SPEED);
  (voice, speed)
}

/// Master TTS on/off switch. Resolution order, highest priority first:
///   1. `HYGG_TTS` — the runtime override (`1/0`, `on/off`, `true/false`).
///   2. `ENABLE_TTS` — environment, then `~/.config/hygg/.env` if present (what
///      `--tts` persists), read the same lightweight way as `tts_settings`.
///   3. the compiled-in default `cfg!(feature = "tts")`: a build made with
///      `--features tts` narrates by default; one without it does not (and its
///      narration code is not compiled in at all).
pub fn tts_enabled_setting() -> bool {
  if let Some(over) = hygg_shared::parse_bool_env("HYGG_TTS") {
    return over;
  }
  let file_values = get_config_env_path()
    .ok()
    .and_then(|path| dotenvy::from_path_iter(path).ok())
    .map(|iter| iter.filter_map(Result::ok).collect::<HashMap<_, _>>())
    .unwrap_or_default();
  config_bool("ENABLE_TTS", &file_values).unwrap_or(cfg!(feature = "tts"))
}

/// OSC 52 clipboard forwarding on yank (`ENABLE_OSC52`). On by default: it is
/// how a yank reaches the outermost terminal's clipboard across an SSH session,
/// where the local clipboard library cannot. Turning it off keeps a yank inside
/// the process's own machine — the setting exists because a document's contents
/// are not always something the reader should be allowed to place on the
/// clipboard of whatever terminal is at the end of the chain.
pub const DEFAULT_OSC52_ENABLED: bool = true;

/// Whether a yank may emit OSC 52. Read the same lightweight way as
/// `tts_enabled_setting`; falls back to `DEFAULT_OSC52_ENABLED`.
pub fn osc52_enabled_setting() -> bool {
  let file_values = get_config_env_path()
    .ok()
    .and_then(|path| dotenvy::from_path_iter(path).ok())
    .map(|iter| iter.filter_map(Result::ok).collect::<HashMap<_, _>>())
    .unwrap_or_default();
  config_bool("ENABLE_OSC52", &file_values).unwrap_or(DEFAULT_OSC52_ENABLED)
}

pub fn save_config(
  config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
  let existing = load_config();
  let merged = |new: Option<bool>, old: Option<bool>, default: bool| {
    new.or(old).unwrap_or(default)
  };
  let managed = [
    (
      "ENABLE_TUTORIAL",
      merged(config.enable_tutorial, existing.enable_tutorial, true)
        .to_string(),
    ),
    (
      "ENABLE_LINE_HIGHLIGHTER",
      merged(
        config.enable_line_highlighter,
        existing.enable_line_highlighter,
        true,
      )
      .to_string(),
    ),
    (
      "SHOW_CURSOR",
      merged(config.show_cursor, existing.show_cursor, true).to_string(),
    ),
    (
      "SHOW_PROGRESS",
      merged(config.show_progress, existing.show_progress, true).to_string(),
    ),
    ("PDF_OCR", merged(config.pdf_ocr, existing.pdf_ocr, false).to_string()),
    (
      "ENABLE_TTS",
      merged(config.tts_enabled, existing.tts_enabled, DEFAULT_TTS_ENABLED)
        .to_string(),
    ),
    (
      "TUTORIAL_SHOWN",
      merged(config.tutorial_shown, existing.tutorial_shown, false).to_string(),
    ),
  ];
  write_env_preserving(&managed)
}

/// Server/sync settings persisted in `~/.config/hygg/.env`. Kept separate from
/// `AppConfig` so the connect/auth/sync commands can read and rewrite just
/// these keys without disturbing the reader's display settings.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
  pub server_url: Option<String>,
  /// The account username (email). Sent with the token on every request; the
  /// server rejects a token whose owner's email does not match, so the token
  /// alone is not a usable credential.
  pub username: Option<String>,
  pub api_token: Option<String>,
  /// Master sync switch (`SYNC`). `false` = fully serverless (no engine, no
  /// reconcile, no manual sync); overrides `auto_sync` entirely.
  pub sync_enabled: bool,
  /// Automatic-sync scope (`AUTO_SYNC`): which documents sync without being
  /// touched. `books` (the default) syncs book-like documents; `all` syncs
  /// everything; `manual` syncs only per-document opt-ins.
  pub auto_sync: AutoSyncPolicy,
  pub device_id: Option<String>,
}

pub fn load_server_config() -> ServerConfig {
  let file_values = get_config_env_path()
    .ok()
    .and_then(|path| dotenvy::from_path_iter(path).ok())
    .map(|iter| iter.filter_map(Result::ok).collect::<HashMap<_, _>>())
    .unwrap_or_default();
  ServerConfig {
    server_url: nonempty(config_string("SERVER_URL", &file_values)),
    username: nonempty(config_string("USERNAME", &file_values)),
    api_token: nonempty(config_string("API_TOKEN", &file_values)),
    sync_enabled: config_bool("SYNC", &file_values)
      .unwrap_or(DEFAULT_SYNC_ENABLED),
    auto_sync: config_string("AUTO_SYNC", &file_values)
      .map(|v| AutoSyncPolicy::from_token_or_default(&v))
      .unwrap_or_default(),
    device_id: nonempty(config_string("DEVICE_ID", &file_values)),
  }
}

pub fn save_server_config(
  config: &ServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
  let managed = [
    ("SERVER_URL", config.server_url.clone().unwrap_or_default()),
    ("USERNAME", config.username.clone().unwrap_or_default()),
    ("API_TOKEN", config.api_token.clone().unwrap_or_default()),
    ("SYNC", config.sync_enabled.to_string()),
    ("AUTO_SYNC", config.auto_sync.to_string()),
    ("DEVICE_ID", config.device_id.clone().unwrap_or_default()),
  ];
  write_env_preserving(&managed)
}

fn nonempty(value: Option<String>) -> Option<String> {
  value.filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn nonempty_filters_blank_values() {
    assert_eq!(nonempty(Some("  ".to_string())), None);
    assert_eq!(nonempty(Some("x".to_string())), Some("x".to_string()));
    assert_eq!(nonempty(None), None);
  }

  #[test]
  fn sync_enabled_default_is_on() {
    const { assert!(DEFAULT_SYNC_ENABLED) };
  }

  #[test]
  fn auto_sync_default_is_books() {
    assert_eq!(AutoSyncPolicy::default(), AutoSyncPolicy::Books);
  }
}
