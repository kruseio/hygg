//! The async actions behind the encryption wizard: sync the account marker,
//! turn encryption on (generating a key), adopt an existing key on this
//! browser, and convert already-uploaded documents. Kept separate from the
//! view so each file stays within the LOC budget.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hygg_shared::crypto::{self, ALG_XCHACHA20POLY1305, KDF_ARGON2ID};
use hygg_shared::sync::proto::EnableEncryptionRequest;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::SettingsCtx;
use crate::sync;

/// On mount, mirror the account's marker into local settings so a browser that
/// joins an already-encrypted account learns it needs the wizard. Never clears
/// a locally-held key; only reflects the account flag + public material.
pub(super) fn sync_marker(settings: SettingsCtx) {
  let Some(creds) = settings.with(|s| s.creds()) else {
    return;
  };
  let had_key = creds.key.is_some();
  spawn_local(async move {
    let Ok(state) = sync::get_encryption(&creds).await else {
      return;
    };
    // Account disabled from elsewhere (e.g. the server toggle) while this
    // browser still holds the key: decrypt our documents back to plaintext and
    // forget the key.
    if !state.enabled && had_key {
      settings.update(|s| {
        s.encryption_enabled = false;
        s.encryption_key = None;
        s.encryption_salt = None;
        s.encryption_verifier = None;
      });
      settings.with(|s| s.save());
      if let Some(c) = settings.with(|s| s.creds()) {
        let _ = sync::convert_library(&c).await;
      }
      return;
    }
    settings.update(|s| {
      s.encryption_enabled = state.enabled;
      if state.enabled {
        s.encryption_salt = Some(state.salt);
        s.encryption_verifier = Some(state.verifier);
      }
    });
    settings.with(|s| s.save());
  });
}

/// Generate a fresh account key, enable encryption on the server, and store the
/// setup locally. Surfaces the generated key via `generated` so the view can
/// show the "save it in your password manager" step.
pub(super) fn turn_on(
  settings: SettingsCtx,
  generated: RwSignal<Option<String>>,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let Some(creds) = settings.with(|s| s.creds()) else {
    status.set("Connect to a server first.".to_string());
    return;
  };
  busy.set(true);
  status.set("Turning on encryption\u{2026}".to_string());
  spawn_local(async move {
    // An account that is enabled *and already initialized* (non-empty salt)
    // must be joined with the existing key, not re-keyed. A server-mandated
    // marker with an empty salt falls through to key generation here.
    if let Ok(state) = sync::get_encryption(&creds).await
      && state.enabled
      && !state.salt.is_empty()
    {
      settings.update(|s| {
        s.encryption_enabled = true;
        s.encryption_salt = Some(state.salt);
        s.encryption_verifier = Some(state.verifier);
      });
      settings.with(|s| s.save());
      status
        .set("This account is already encrypted — enter its key below.".into());
      busy.set(false);
      return;
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
        status.set(format!("Couldn't generate a key: {e}"));
        busy.set(false);
        return;
      }
    };
    let req = EnableEncryptionRequest {
      kdf: KDF_ARGON2ID.to_string(),
      alg: ALG_XCHACHA20POLY1305,
      salt: salt_b64.clone(),
      verifier: verifier_b64.clone(),
    };
    match sync::enable_encryption(&creds, &req).await {
      Ok(_) => {
        settings.update(|s| {
          s.encryption_enabled = true;
          s.encryption_key = Some(phrase.clone());
          s.encryption_salt = Some(salt_b64);
          s.encryption_verifier = Some(verifier_b64);
        });
        settings.with(|s| s.save());
        generated.set(Some(phrase));
        status.set(String::new());
        // Seal any already-uploaded plaintext documents in the background.
        if let Some(c) = settings.with(|s| s.creds()) {
          spawn_local(async move {
            let _ = sync::convert_library(&c).await;
          });
        }
      }
      Err(e) => status.set(format!("The server refused: {e}")),
    }
    busy.set(false);
  });
}

/// Adopt an existing account key on this browser after confirming it against
/// the account verifier.
pub(super) fn adopt_key(
  settings: SettingsCtx,
  key_input: RwSignal<String>,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let secret = key_input.get().trim().to_string();
  if secret.is_empty() {
    status.set("Paste your account key.".to_string());
    return;
  }
  let Some(creds) = settings.with(|s| s.creds()) else {
    status.set("Connect to a server first.".to_string());
    return;
  };
  busy.set(true);
  status.set("Checking key\u{2026}".to_string());
  spawn_local(async move {
    match sync::get_encryption(&creds).await {
      Ok(state) if state.enabled => {
        let ok = (|| {
          let salt = STANDARD.decode(&state.salt).ok()?;
          let key = crypto::derive_key(secret.as_bytes(), &salt).ok()?;
          let verifier = STANDARD.decode(&state.verifier).ok()?;
          Some(crypto::check_verifier(&key, &verifier))
        })();
        if ok == Some(true) {
          settings.update(|s| {
            s.encryption_enabled = true;
            s.encryption_key = Some(secret.clone());
            s.encryption_salt = Some(state.salt);
            s.encryption_verifier = Some(state.verifier);
          });
          settings.with(|s| s.save());
          key_input.set(String::new());
          status.set(
            "\u{2713} Key accepted. Encrypted documents now sync here.".into(),
          );
        } else {
          status.set("That key is not correct for this account.".to_string());
        }
      }
      Ok(_) => status.set("This account doesn't use encryption yet.".into()),
      Err(e) => status.set(format!("Couldn't reach the server: {e}")),
    }
    busy.set(false);
  });
}

/// Re-upload every locally-stored document sealed under the account key.
pub(super) fn convert(
  settings: SettingsCtx,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let Some(creds) = settings.with(|s| s.creds()) else {
    status.set("Connect to a server first.".to_string());
    return;
  };
  if creds.key.is_none() {
    status.set("Set up encryption on this browser first.".to_string());
    return;
  }
  busy.set(true);
  status.set("Encrypting your documents\u{2026}".to_string());
  spawn_local(async move {
    match sync::convert_library(&creds).await {
      Ok((sealed, skipped)) => {
        status.set(format!("Done. Sealed {sealed}, skipped {skipped}."))
      }
      Err(e) => status.set(format!("Conversion failed: {e}")),
    }
    busy.set(false);
  });
}

/// Turn encryption off for the whole account, then re-upload this browser's
/// documents as plaintext in the background so the server holds readable bytes
/// again. Reading is never interrupted.
pub(super) fn disable(
  settings: SettingsCtx,
  status: RwSignal<String>,
  busy: RwSignal<bool>,
) {
  let Some(creds) = settings.with(|s| s.creds()) else {
    status.set("Connect to a server first.".to_string());
    return;
  };
  busy.set(true);
  status.set("Turning off encryption\u{2026}".to_string());
  spawn_local(async move {
    if let Err(e) = sync::disable_encryption(&creds).await {
      status.set(format!("Couldn't turn it off: {e}"));
      busy.set(false);
      return;
    }
    // Forget the key so the re-upload below (and every later upload) is
    // plaintext now that the account no longer requires encryption.
    settings.update(|s| {
      s.encryption_enabled = false;
      s.encryption_key = None;
      s.encryption_salt = None;
      s.encryption_verifier = None;
    });
    settings.with(|s| s.save());
    if let Some(plain_creds) = settings.with(|s| s.creds()) {
      let _ = sync::convert_library(&plain_creds).await;
    }
    status.set("Encryption off. Your documents were decrypted.".to_string());
    busy.set(false);
  });
}

/// Clear this browser's key + settings (the account and other devices are
/// untouched).
pub(super) fn forget(settings: SettingsCtx, status: RwSignal<String>) {
  settings.update(|s| {
    s.encryption_enabled = false;
    s.encryption_key = None;
    s.encryption_salt = None;
    s.encryption_verifier = None;
  });
  settings.with(|s| s.save());
  status.set("Cleared this browser's encryption key.".to_string());
}
