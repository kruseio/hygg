//! Desktop entry point. Mobile enters via the generated entry point in the lib
//! (`#[tauri::mobile_entry_point]` on `run`), so all app logic lives in
//! `lib.rs` and both targets share it.
//!
//! The `windows_subsystem = "windows"` attribute suppresses the extra console
//! window on Windows release builds (no effect on macOS/Linux).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  hygg_tauri_lib::run();
}
