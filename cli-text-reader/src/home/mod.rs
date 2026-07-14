//! The `:home` landing view: `run_home`, the full-screen picker shown when
//! hygg starts with no input file (select a document to resume). The in-reader
//! `:home` / `:Rex` commands leave the current document and return here, so
//! both entry points land on exactly the same view. Works fully offline
//! against the local library index.

mod command;
mod download;
mod picker;
mod render;
mod sync;

pub use picker::run_home;
pub use render::load_home_items;
pub use sync::reconcile_home_items;

/// Whether a no-input launch should open the home view. We suppress it only for
/// a brand-new user (empty library and the first-run tutorial not yet seen) so
/// `hygg` with no args still onboards them with the tutorial; everyone else
/// gets home as the default landing view.
pub fn should_show_home() -> bool {
  let has_library = !crate::library::load_index().is_empty();
  let tutorial_shown =
    crate::config::load_config().tutorial_shown.unwrap_or(false);
  has_library || tutorial_shown
}
