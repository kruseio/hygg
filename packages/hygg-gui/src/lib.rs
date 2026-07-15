//! hygg-gui — the hygg reader as a native iced desktop GUI (Windows, Linux,
//! macOS).
//!
//! The reading experience mirrors `hygg-pwa` (which is the browser reader) — a
//! justified monospace column, a touch-first top bar, a home library dashboard,
//! and offline-first storage — but rendered natively so the binary can be the
//! system's default document reader.
//!
//! Entry points live in [`app::launch`]; `main.rs` is a thin shim over it.

mod ansi;
mod app;
mod assets;
mod build_info;
mod credits;
mod format;
mod icons;
mod layout;
mod model;
mod screens;
mod select;
mod settings;
mod storage;
mod sync;
mod theme;
mod util;
mod widget;

pub use app::launch;
