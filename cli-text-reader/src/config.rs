use crate::utils::{
  ensure_config_file_with_defaults, get_hygg_config_file, parse_bool_env_var,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Default)]
pub struct AppConfig {
  pub enable_tutorial: Option<bool>,
  pub enable_line_highlighter: Option<bool>,
  pub show_cursor: Option<bool>,
  pub show_progress: Option<bool>,
  pub pdf_ocr: Option<bool>,
  pub tutorial_shown: Option<bool>,
}

fn get_config_env_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
  get_hygg_config_file(".env")
}

fn ensure_config_file() -> Result<(), Box<dyn std::error::Error>> {
  let config_path = get_config_env_path()?;
  ensure_config_file_with_defaults(
    &config_path,
    "ENABLE_TUTORIAL=true\nENABLE_LINE_HIGHLIGHTER=true\nSHOW_CURSOR=true\nSHOW_PROGRESS=true\nPDF_OCR=false\nTUTORIAL_SHOWN=false\n",
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

pub fn save_config(
  config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
  let config_path = get_config_env_path()?;

  let existing_config = load_config();

  let enable_tutorial =
    config.enable_tutorial.or(existing_config.enable_tutorial).unwrap_or(true);
  let enable_line_highlighter = config
    .enable_line_highlighter
    .or(existing_config.enable_line_highlighter)
    .unwrap_or(true);
  let show_cursor =
    config.show_cursor.or(existing_config.show_cursor).unwrap_or(true);
  let show_progress =
    config.show_progress.or(existing_config.show_progress).unwrap_or(true);
  let pdf_ocr = config.pdf_ocr.or(existing_config.pdf_ocr).unwrap_or(false);
  let tutorial_shown =
    config.tutorial_shown.or(existing_config.tutorial_shown).unwrap_or(false);

  let content = format!(
    "ENABLE_TUTORIAL={enable_tutorial}\nENABLE_LINE_HIGHLIGHTER={enable_line_highlighter}\nSHOW_CURSOR={show_cursor}\nSHOW_PROGRESS={show_progress}\nPDF_OCR={pdf_ocr}\nTUTORIAL_SHOWN={tutorial_shown}\n"
  );

  fs::write(config_path, content)?;
  Ok(())
}
