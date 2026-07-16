use crate::config;

pub fn load_ocr_enabled_config() -> bool {
  config::load_config().pdf_ocr.unwrap_or(false)
}

/// The saved OCR preference (`PDF_OCR` env or the config file), or `None` when
/// unset. `load_ocr_enabled_config` collapses `None` to `false`; `main` needs
/// the distinction so it can fall back to the `HYGG_OCR` override and then to
/// the compiled-in `ocr` feature default instead of a hardcoded `false`.
pub fn ocr_enabled_config_opt() -> Option<bool> {
  config::load_config().pdf_ocr
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
