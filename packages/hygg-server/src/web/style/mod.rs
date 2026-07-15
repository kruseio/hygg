mod base;
mod components;
mod docs;
mod responsive;

use std::sync::RwLock;

pub(crate) use base::APP_CSS_BASE;
pub(crate) use components::APP_CSS_COMPONENTS;
pub(crate) use docs::APP_CSS_DOCS;
pub(crate) use responsive::APP_CSS_RESPONSIVE;

/// CSS an extension asked to have appended to the core stylesheet.
///
/// Process-wide rather than carried on `AppState`, because `page` renders
/// signed-out pages too — there is no user and no state in scope there to hang
/// it off, and a deployment's styling is a single constant either way.
static EXTRA_CSS: RwLock<&'static str> = RwLock::new("");

/// Install the extension's CSS. Called when a web extension is installed.
pub(crate) fn set_extra_css(css: &'static str) {
  if let Ok(mut slot) = EXTRA_CSS.write() {
    *slot = css;
  }
}

/// The extension's CSS, or nothing when no extension asked for any.
pub(crate) fn extra_css() -> &'static str {
  EXTRA_CSS.read().map(|slot| *slot).unwrap_or("")
}
