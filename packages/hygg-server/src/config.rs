//! Server configuration, loaded from environment variables (autoloaded from a
//! `.env` file by `dotenvy` in `main`). See `.env.example` for the full list.

use std::env;

#[derive(Clone, Debug)]
pub struct Config {
  /// Path to the SQLite database, e.g. `sqlite://data/hygg-server.db`.
  pub database_url: String,
  /// Address to bind the HTTP server to.
  pub bind_addr: String,
  /// Maximum accepted request body size in bytes. Caps blob uploads (the
  /// largest payload) and rejects oversized requests with 413 before they are
  /// buffered. Defaults to 128 MiB.
  pub max_body_bytes: usize,
  /// WebAuthn relying-party id, usually the bare host name.
  pub rp_id: String,
  /// WebAuthn relying-party origin, including scheme and port.
  pub rp_origin: String,
  /// Human-readable relying-party name shown by passkey authenticators.
  pub rp_name: String,
  /// Optional total server storage budget in bytes. When set, admins are
  /// warned as total stored document bytes approach/exceed it. `None` =
  /// unlimited.
  pub server_storage_limit_bytes: Option<i64>,
  /// Public URL of the browser PWA. Used as a CORS allow-list default, and
  /// available to any page that wants to link readers to it.
  pub pwa_url: String,
  /// Exact origins allowed to call the JSON API cross-origin (the PWA runs on
  /// a separate origin, so a CORS allow-list is required). Comma-separated in
  /// `CORS_ALLOW_ORIGINS`; defaults to `pwa_url` plus the localhost dev
  /// origins.
  pub cors_allow_origins: Vec<String>,
  /// Base directory for rotating log files. The server writes to a per-service
  /// subdirectory (`<log_dir>/hygg-server`). Defaults to `data/logs` (under
  /// the project's `./data` tree); override with `LOG_DIR`.
  pub log_dir: String,
  /// Whether to cache and reuse server-side document extraction (`/convert`
  /// results, and a background pre-warm on blob upload). On (default), an
  /// expensive OCR/pandoc conversion runs once per `(document, width)` and is
  /// reused by every later import; off, every conversion re-runs the pipeline
  /// and nothing is written to `book_extractions`. Toggle with
  /// `HYGG_EXTRACTION_CACHE` (a kill switch if the cache misbehaves).
  pub extraction_cache: bool,
  /// Maximum document conversions (`/convert` and the upload pre-warm) running
  /// at once. Each pins a blocking thread for seconds — OCR, or the `pandoc`
  /// child process — so an unbounded fan-out of hostile uploads would exhaust
  /// the blocking pool and the host's CPU while every other request waits.
  /// Callers past the limit get 429 rather than queueing (a queued request
  /// keeps its whole body buffered). Override with `HYGG_CONVERT_CONCURRENCY`.
  pub convert_concurrency: usize,
}

/// Default maximum request body size (128 MiB), large enough for document
/// uploads.
pub const DEFAULT_MAX_BODY_BYTES: usize = 128 * 1024 * 1024;

/// Default number of document conversions allowed to run concurrently.
pub const DEFAULT_CONVERT_CONCURRENCY: usize = 2;

/// Default interface to bind: all interfaces, so the server is reachable on the
/// LAN. Override with `HOST` (or set `127.0.0.1` for localhost-only).
pub const DEFAULT_HOST: &str = "0.0.0.0";
/// Default listen port. Chosen to be unlikely to clash with common services.
/// Override with `PORT`.
pub const DEFAULT_PORT: &str = "3032";
/// Default base directory for rotating logs, under the project's `./data` tree.
/// Override with `LOG_DIR`.
pub const DEFAULT_LOG_DIR: &str = "data/logs";

impl Config {
  pub fn from_env() -> Self {
    Self {
      database_url: env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/hygg-server.db".to_string()),
      bind_addr: bind_addr_from_env(),
      max_body_bytes: env::var("MAX_BODY_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_BYTES),
      rp_id: env_nonempty("RP_ID").unwrap_or_else(|| "localhost".to_string()),
      rp_origin: env_nonempty("RP_ORIGIN")
        .unwrap_or_else(|| "http://localhost:3032".to_string()),
      rp_name: env_nonempty("RP_NAME").unwrap_or_else(|| "hygg".to_string()),
      server_storage_limit_bytes: env_nonempty("SERVER_STORAGE_LIMIT_BYTES")
        .and_then(|v| v.parse().ok())
        .filter(|&v| v > 0),
      pwa_url: env_nonempty("PWA_URL")
        .unwrap_or_else(|| "https://pwa.hygg.kruseio.com".to_string()),
      cors_allow_origins: cors_origins_from_env(),
      log_dir: env_nonempty("LOG_DIR")
        .unwrap_or_else(|| DEFAULT_LOG_DIR.to_string()),
      extraction_cache: env_bool("HYGG_EXTRACTION_CACHE", true),
      convert_concurrency: env_nonempty("HYGG_CONVERT_CONCURRENCY")
        .and_then(|v| v.parse().ok())
        .filter(|&v: &usize| v > 0)
        .unwrap_or(DEFAULT_CONVERT_CONCURRENCY),
    }
  }
}

/// Parse the CORS allow-list from `CORS_ALLOW_ORIGINS` (comma-separated exact
/// origins). When unset, allow any origin: the self-host server binds the LAN
/// (0.0.0.0) and the PWA can be served from any address on it (e.g.
/// `http://<lan-ip>:8080`). This is safe because the JSON API is
/// bearer-authenticated with no cookies — CORS can't protect a header token,
/// and a strict list only breaks the LAN-served PWA. A locked-down deployment
/// (one that tightens this default in its own composition) sets
/// `CORS_ALLOW_ORIGINS` explicitly.
fn cors_origins_from_env() -> Vec<String> {
  if let Some(raw) = env_nonempty("CORS_ALLOW_ORIGINS") {
    return raw
      .split(',')
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
      .collect();
  }
  vec!["*".to_string()]
}

/// Resolve the listen address from the environment. A full `BIND_ADDR`
/// (`host:port`) wins when set; otherwise it is composed from `HOST` and
/// `PORT`, each with a sensible default — so changing just `PORT=…` in `.env`
/// is enough to move the server to another port.
fn bind_addr_from_env() -> String {
  if let Some(addr) = env_nonempty("BIND_ADDR") {
    return addr;
  }
  let host = env_nonempty("HOST").unwrap_or_else(|| DEFAULT_HOST.to_string());
  let port = env_nonempty("PORT").unwrap_or_else(|| DEFAULT_PORT.to_string());
  format!("{host}:{port}")
}

/// Read an env var, treating unset and blank (e.g. `PORT=` in `.env`) alike so
/// an empty value falls back to the default rather than producing `host:`.
fn env_nonempty(key: &str) -> Option<String> {
  env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Read a boolean env var (`1`/`true`/`yes`/`on` = true, `0`/`false`/`no`/`off`
/// = false, case-insensitive). Unset, blank, or unrecognised values fall back
/// to `default`.
fn env_bool(key: &str, default: bool) -> bool {
  match env_nonempty(key).map(|v| v.trim().to_ascii_lowercase()) {
    Some(v) if matches!(v.as_str(), "1" | "true" | "yes" | "on") => true,
    Some(v) if matches!(v.as_str(), "0" | "false" | "no" | "off") => false,
    _ => default,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Remove every env var this module reads so each assertion starts clean.
  /// Kept in one test (env is process-global) to avoid races between parallel
  /// tests mutating the same vars.
  fn clear_bind_env() {
    unsafe {
      env::remove_var("DATABASE_URL");
      env::remove_var("BIND_ADDR");
      env::remove_var("HOST");
      env::remove_var("PORT");
      env::remove_var("RP_ID");
      env::remove_var("RP_ORIGIN");
      env::remove_var("RP_NAME");
      env::remove_var("PWA_URL");
      env::remove_var("CORS_ALLOW_ORIGINS");
      env::remove_var("LOG_DIR");
      env::remove_var("HYGG_EXTRACTION_CACHE");
    }
  }

  // Env is process-global, so all the env-dependent assertions live in one
  // test — parallel tests mutating the same vars would race.
  #[test]
  fn config_resolves_from_env() {
    clear_bind_env();

    // Defaults when nothing is set.
    let config = Config::from_env();
    assert_eq!(config.database_url, "sqlite://data/hygg-server.db");
    assert_eq!(config.log_dir, "data/logs");
    assert_eq!(config.bind_addr, "0.0.0.0:3032");
    assert_eq!(config.max_body_bytes, DEFAULT_MAX_BODY_BYTES);
    assert_eq!(config.rp_id, "localhost");
    assert_eq!(config.rp_origin, "http://localhost:3032");
    assert_eq!(config.rp_name, "hygg");
    assert_eq!(config.pwa_url, "https://pwa.hygg.kruseio.com");
    // Self-host default: any origin, so a LAN-served PWA works without config.
    assert_eq!(config.cors_allow_origins, vec!["*".to_string()]);
    // Extraction cache is on by default.
    assert!(config.extraction_cache);

    // The kill switch turns it off (and recognises common truthy/falsy forms).
    unsafe { env::set_var("HYGG_EXTRACTION_CACHE", "false") };
    assert!(!Config::from_env().extraction_cache);
    unsafe { env::set_var("HYGG_EXTRACTION_CACHE", "off") };
    assert!(!Config::from_env().extraction_cache);
    unsafe { env::set_var("HYGG_EXTRACTION_CACHE", "1") };
    assert!(Config::from_env().extraction_cache);
    unsafe { env::remove_var("HYGG_EXTRACTION_CACHE") };

    // An explicit allow-list always wins.
    unsafe {
      env::set_var(
        "CORS_ALLOW_ORIGINS",
        "https://a.example, https://b.example",
      );
    }
    assert_eq!(
      Config::from_env().cors_allow_origins,
      vec!["https://a.example".to_string(), "https://b.example".to_string()]
    );
    unsafe {
      env::remove_var("CORS_ALLOW_ORIGINS");
    }

    // PORT alone moves the port; HOST keeps its default.
    unsafe { env::set_var("PORT", "9090") };
    assert_eq!(Config::from_env().bind_addr, "0.0.0.0:9090");

    // HOST narrows the interface.
    unsafe { env::set_var("HOST", "127.0.0.1") };
    assert_eq!(Config::from_env().bind_addr, "127.0.0.1:9090");

    // A blank value falls back to the default rather than producing "host:".
    unsafe { env::set_var("PORT", "") };
    assert_eq!(Config::from_env().bind_addr, "127.0.0.1:3032");

    // A full BIND_ADDR overrides HOST/PORT entirely.
    unsafe { env::set_var("BIND_ADDR", "0.0.0.0:7000") };
    assert_eq!(Config::from_env().bind_addr, "0.0.0.0:7000");

    unsafe {
      env::set_var("RP_ID", "reader.example.com");
      env::set_var("RP_ORIGIN", "https://reader.example.com");
      env::set_var("RP_NAME", "hygg reader");
    }
    let config = Config::from_env();
    assert_eq!(config.rp_id, "reader.example.com");
    assert_eq!(config.rp_origin, "https://reader.example.com");
    assert_eq!(config.rp_name, "hygg reader");

    clear_bind_env();
  }
}
