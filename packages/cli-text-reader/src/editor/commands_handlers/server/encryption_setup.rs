//! The acting half of the `:encryption` wizard: generate a key and turn
//! encryption on, adopt an existing key on a new device, and convert documents
//! that were uploaded before encryption was enabled.

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hygg_shared::crypto::{self, ALG_XCHACHA20POLY1305, KDF_ARGON2ID};
use hygg_shared::sync::proto::EnableEncryptionRequest;

use super::encryption::{marker_with_timeout, not_connected_lines};
use crate::config::{
  ENCRYPTION_KEY_ENV, EncryptionConfig, load_encryption_config,
  load_server_config, save_encryption_config,
};
use crate::editor::core::Editor;
use crate::sync::SyncClient;

fn dismiss(mut lines: Vec<String>) -> Vec<String> {
  lines.push("  :q to dismiss".to_string());
  lines
}

impl Editor {
  /// `:encryption setup` — generate a fresh account key, turn encryption on
  /// for the account, and save the setup on this device. Refuses if the
  /// account is already encrypted (that is the `:encryption use` path).
  pub(super) fn encryption_setup(&mut self) -> Vec<String> {
    let Some(client) = SyncClient::from_config(&load_server_config()) else {
      return not_connected_lines();
    };
    // Refuse only when the account is *initialized* (a key already exists); a
    // server-mandated marker with no key yet (empty salt) is exactly what this
    // command initializes.
    if matches!(marker_with_timeout(), Some(s) if s.enabled && !s.salt.is_empty())
    {
      return dismiss(vec![
        "  This account is already encrypted.".to_string(),
        "  Set this device up with the existing key instead:".to_string(),
        "    :encryption use <key>".to_string(),
      ]);
    }
    let built = (|| {
      let phrase = crypto::generate_key_phrase()?;
      let salt = crypto::random_salt()?;
      let key = crypto::derive_key(phrase.as_bytes(), &salt)?;
      let verifier = crypto::make_verifier(&key)?;
      Ok::<_, crypto::CryptoError>((
        phrase,
        STANDARD.encode(salt),
        STANDARD.encode(verifier),
      ))
    })();
    let (phrase, salt_b64, verifier_b64) = match built {
      Ok(v) => v,
      Err(e) => {
        return dismiss(vec![format!("  Couldn't generate a key: {e}")]);
      }
    };
    let req = EnableEncryptionRequest {
      kdf: KDF_ARGON2ID.to_string(),
      alg: ALG_XCHACHA20POLY1305,
      salt: salt_b64.clone(),
      verifier: verifier_b64.clone(),
    };
    match enable_with_timeout(client, req) {
      Some(Ok(_)) => {
        let cfg = EncryptionConfig {
          enabled: true,
          secret: Some(phrase.clone()),
          salt_b64: Some(salt_b64),
          verifier_b64: Some(verifier_b64),
        };
        if let Err(e) = save_encryption_config(&cfg, true) {
          return dismiss(vec![format!("  Enabled, but couldn't save: {e}")]);
        }
        // Seal any already-uploaded plaintext documents in the background —
        // reading is never interrupted.
        spawn_background_convert();
        setup_success_lines(&phrase)
      }
      Some(Err(e)) => dismiss(vec![format!("  The server refused: {e}")]),
      None => dismiss(vec!["  The server didn't respond in time.".to_string()]),
    }
  }

  /// `:encryption use <key>` — adopt the account's existing key on this device
  /// after confirming it against the account verifier.
  pub(super) fn encryption_use(&mut self, key_input: String) -> Vec<String> {
    let secret = key_input.trim().to_string();
    if secret.is_empty() {
      return dismiss(vec!["  Provide the key: :encryption use <key>".into()]);
    }
    let Some(state) = marker_with_timeout() else {
      return not_connected_lines();
    };
    if !state.enabled {
      return dismiss(vec![
        "  This account doesn't use encryption yet.".to_string(),
        "  Turn it on with :encryption setup.".to_string(),
      ]);
    }
    let verified = (|| {
      let salt = STANDARD.decode(&state.salt).ok()?;
      let key = crypto::derive_key(secret.as_bytes(), &salt).ok()?;
      let verifier = STANDARD.decode(&state.verifier).ok()?;
      Some(crypto::check_verifier(&key, &verifier))
    })();
    if verified != Some(true) {
      return dismiss(vec![
        "  That key is not correct for this account.".to_string(),
        "  Check your password manager and try again.".to_string(),
      ]);
    }
    let cfg = EncryptionConfig {
      enabled: true,
      secret: Some(secret),
      salt_b64: Some(state.salt),
      verifier_b64: Some(state.verifier),
    };
    match save_encryption_config(&cfg, true) {
      Ok(()) => dismiss(vec![
        "  ✓ Key accepted and saved on this device.".to_string(),
        "  Encrypted documents and notes now sync here.".to_string(),
        format!(
          "  Tip: also set {ENCRYPTION_KEY_ENV} in your shell so the key isn't"
        ),
        "  only in the config file.".to_string(),
      ]),
      Err(e) => dismiss(vec![format!("  Couldn't save: {e}")]),
    }
  }

  /// `:encryption convert` — re-upload every already-plaintext document sealed.
  pub(super) fn encryption_convert(&mut self) -> Vec<String> {
    if load_encryption_config().resolve_key().is_none() {
      return dismiss(vec![
        "  Encryption isn't set up on this device yet.".to_string(),
        "  Run :encryption setup or :encryption use <key> first.".to_string(),
      ]);
    }
    let Some(client) = SyncClient::from_config(&load_server_config()) else {
      return not_connected_lines();
    };
    let (tx, rx) = channel();
    thread::spawn(move || {
      let _ = tx.send(convert_library(&client));
    });
    match rx.recv_timeout(Duration::from_secs(180)) {
      Ok(Ok((sealed, skipped, failed))) => dismiss(vec![
        "  Conversion complete.".to_string(),
        format!("    sealed:  {sealed}"),
        format!(
          "    skipped: {skipped} (already encrypted or bytes not synced)"
        ),
        format!("    failed:  {failed}"),
      ]),
      Ok(Err(e)) => dismiss(vec![format!("  Conversion failed: {e}")]),
      Err(_) => dismiss(vec![
        "  Still converting in the background.".to_string(),
        "  Re-run :encryption later to check the state.".to_string(),
      ]),
    }
  }

  /// `:encryption disable` — turn encryption off for the account, then decrypt
  /// every document and re-upload it as plaintext in the background (reading is
  /// not interrupted), and forget this device's key.
  pub(super) fn encryption_disable(&mut self) -> Vec<String> {
    if load_encryption_config().resolve_key().is_none() {
      return dismiss(vec![
        "  Encryption isn't set up on this device, so there's no key to"
          .to_string(),
        "  decrypt your documents with. Set it up first, or use another device."
          .to_string(),
      ]);
    }
    let Some(client) = SyncClient::from_config(&load_server_config()) else {
      return not_connected_lines();
    };
    match disable_with_timeout(client) {
      Some(Ok(_)) => {
        spawn_background_decrypt();
        dismiss(vec![
          "  Encryption turned off for your account.".to_string(),
          "  Decrypting your documents and re-uploading them as plaintext in"
            .to_string(),
          "  the background — reading is not interrupted.".to_string(),
        ])
      }
      Some(Err(e)) => dismiss(vec![format!("  The server refused: {e}")]),
      None => dismiss(vec!["  The server didn't respond in time.".to_string()]),
    }
  }
}

/// Fire-and-forget: seal every already-plaintext document on the server. Used
/// by `setup` so migration never blocks the reader.
fn spawn_background_convert() {
  thread::spawn(|| {
    if let Some(client) = SyncClient::from_config(&load_server_config()) {
      let _ = convert_library(&client);
    }
  });
}

/// Fire-and-forget: decrypt every document, re-upload it as plaintext, then
/// forget the local key (now that nothing needs it). Also used by the
/// reconcile path when the account is disabled from elsewhere (the server).
pub(super) fn spawn_background_decrypt() {
  thread::spawn(|| {
    if let Some(client) = SyncClient::from_config(&load_server_config()) {
      let _ = client.decrypt_and_reupload_all();
    }
    let _ = save_encryption_config(&EncryptionConfig::default(), true);
  });
}

/// Run `disable_encryption` on a worker thread with a bounded wait.
fn disable_with_timeout(
  client: SyncClient,
) -> Option<Result<hygg_shared::sync::proto::EncryptionState, String>> {
  let (tx, rx) = channel();
  thread::spawn(move || {
    let _ = tx.send(client.disable_encryption());
  });
  rx.recv_timeout(Duration::from_secs(15)).ok()
}

/// The "save your key" screen — the single most important step of setup.
fn setup_success_lines(phrase: &str) -> Vec<String> {
  dismiss(vec![
    "  ━━━ Encryption is ON ━━━".to_string(),
    "  ".to_string(),
    "  YOUR ENCRYPTION KEY — SAVE IT NOW:".to_string(),
    "  ".to_string(),
    format!("      {phrase}"),
    "  ".to_string(),
    "  ► Put this in your password manager. It is the ONLY way to read your"
      .to_string(),
    "    documents. Lose it and the data is unrecoverable — the server"
      .to_string(),
    "    operator cannot help, by design.".to_string(),
    "  ".to_string(),
    "  Saved on this device. On every OTHER device, set:".to_string(),
    format!("      export {ENCRYPTION_KEY_ENV}=\"{phrase}\""),
    "  and run  :encryption use <key>  there.".to_string(),
    "  ".to_string(),
    "  Your existing documents are being sealed in the background now."
      .to_string(),
  ])
}

/// Run `enable_encryption` on a worker thread with a bounded wait.
fn enable_with_timeout(
  client: SyncClient,
  req: EnableEncryptionRequest,
) -> Option<Result<hygg_shared::sync::proto::EncryptionState, String>> {
  let (tx, rx) = channel();
  thread::spawn(move || {
    let _ = tx.send(client.enable_encryption(&req));
  });
  rx.recv_timeout(Duration::from_secs(15)).ok()
}

/// Seal every plaintext blob on the server. Returns `(sealed, skipped,
/// failed)`. A blob that is already an envelope, or a document whose bytes do
/// not sync, is skipped; a per-document error is counted, not fatal.
fn convert_library(
  client: &SyncClient,
) -> Result<(usize, usize, usize), String> {
  let books = client.fetch_library()?;
  let (mut sealed, mut skipped, mut failed) = (0usize, 0usize, 0usize);
  for book in books {
    if !book.sync_mode.syncs_blob() {
      skipped += 1;
      continue;
    }
    match client.download_book_raw(&book.content_hash) {
      Ok(raw) if crypto::is_envelope(&raw) => skipped += 1,
      Ok(raw) => {
        match client.upload_book(
          &book.content_hash,
          &book.title,
          &book.format,
          &raw,
        ) {
          Ok(()) => sealed += 1,
          Err(_) => failed += 1,
        }
      }
      Err(_) => failed += 1,
    }
  }
  Ok((sealed, skipped, failed))
}
