use crate::config;

pub fn load_ocr_enabled_config() -> bool {
  config::load_config().pdf_ocr.unwrap_or(false)
}

pub fn save_ocr_enabled_config(
  enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  config::save_config(&config::AppConfig {
    pdf_ocr: Some(enabled),
    ..Default::default()
  })
}

pub fn save_tts_enabled_config(
  enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
  config::save_config(&config::AppConfig {
    tts_enabled: Some(enabled),
    ..Default::default()
  })
}
