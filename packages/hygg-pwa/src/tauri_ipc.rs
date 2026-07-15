//! Bridge to the native Tauri backend (`hygg-tauri`).
//!
//! When the PWA runs inside the Tauri shell, the heavy extraction pipeline runs
//! as native Rust over IPC instead of in wasm (see the crate root). This module
//! is the thin wasm side of that seam: detect the shell and call its
//! `#[tauri::command]`s.
//!
//! With `withGlobalTauri` enabled in `tauri.conf.json`, Tauri exposes
//! `window.__TAURI__.core.invoke`, so this no-bundler Trunk app can call it
//! directly via `js-sys` without the `@tauri-apps/api` npm package. In a plain
//! browser `window.__TAURI__` is undefined and [`in_tauri`] returns `false`, so
//! the caller falls back to the in-wasm pipeline.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::model::LineKind;

/// The extractor's output, deserialized from the native command's JSON. Mirrors
/// `hygg_tauri::Extracted`; `kinds` deserializes straight into the frontend's
/// [`LineKind`] (same serde representation), so it drops into a `Book` with no
/// translation.
#[derive(Deserialize)]
pub struct Extracted {
  pub id: String,
  pub lines: Vec<String>,
  pub kinds: Vec<LineKind>,
  pub format: String,
  #[serde(default)]
  pub page_starts: Vec<usize>,
}

/// Whether the app is running inside the Tauri shell (native desktop/mobile)
/// rather than a plain browser. Cheap enough to call per import.
pub fn in_tauri() -> bool {
  web_sys::window()
    .and_then(|w| {
      js_sys::Reflect::get(&w, &JsValue::from_str("__TAURI__")).ok()
    })
    .map(|v| !v.is_undefined() && !v.is_null())
    .unwrap_or(false)
}

/// Extract a document's bytes via the native `extract_document` IPC command.
/// Errors (missing bridge, backend failure) come back as a user-facing string,
/// matching the wasm path's `Result<_, String>`.
pub async fn extract_document(
  filename: &str,
  bytes: &[u8],
  col: usize,
) -> Result<Extracted, String> {
  let args = js_sys::Object::new();
  set(&args, "filename", &JsValue::from_str(filename))?;
  // Bytes travel as base64 in a plain string field. A string argument
  // serializes unambiguously across Tauri's IPC (a typed array's mapping to
  // `Vec<u8>` is version-dependent), and base64 is ~1.33x the payload vs the
  // ~3-4x of a JSON number array — the safe, still-compact choice.
  set(&args, "b64", &JsValue::from_str(&STANDARD.encode(bytes)))?;
  set(&args, "col", &JsValue::from_f64(col as f64))?;

  let result = invoke("extract_document", args.into()).await?;
  serde_wasm_bindgen::from_value(result)
    .map_err(|e| format!("Bad extraction response: {e}"))
}

/// Call `window.__TAURI__.core.invoke(cmd, args)` and await its promise.
async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, String> {
  let win = web_sys::window().ok_or("no window")?;
  let tauri = get(&win, "__TAURI__")?;
  let core = get(&tauri, "core")?;
  let invoke_fn = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))
    .ok()
    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
    .ok_or("__TAURI__.core.invoke unavailable")?;
  let promise = invoke_fn
    .call2(&core, &JsValue::from_str(cmd), &args)
    .map_err(|e| jserr("invoke() threw", &e))?
    .dyn_into::<js_sys::Promise>()
    .map_err(|_| "invoke() did not return a promise".to_string())?;
  JsFuture::from(promise).await.map_err(|e| jserr("IPC failed", &e))
}

/// `Reflect::get` a named property, as a `Result<_, String>`.
fn get(obj: &JsValue, key: &str) -> Result<JsValue, String> {
  js_sys::Reflect::get(obj, &JsValue::from_str(key))
    .map_err(|_| format!("missing {key}"))
}

/// `Reflect::set` a named property, mapping any error to a string.
fn set(obj: &js_sys::Object, key: &str, val: &JsValue) -> Result<(), String> {
  js_sys::Reflect::set(obj, &JsValue::from_str(key), val)
    .map(|_| ())
    .map_err(|_| format!("couldn't set {key}"))
}

/// Render a caught JS error into a message, falling back to a label.
fn jserr(label: &str, e: &JsValue) -> String {
  e.as_string()
    .map(|s| format!("{label}: {s}"))
    .unwrap_or_else(|| label.to_string())
}
