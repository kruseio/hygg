//! `:encryption` — the end-to-end encryption setup wizard.
//!
//! A state-aware overlay walks the reader through turning encryption on, and,
//! on a device that joins an already-encrypted account, through pasting the
//! account key. The heavier actions (generate + enable, adopt a key, convert
//! existing documents) live in [`super::encryption_setup`]; this file owns the
//! dispatch, the status screen, "forget key", and the shared network helpers.

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use crate::config::{
  EncryptionConfig, load_encryption_config, load_server_config,
  save_encryption_config,
};
use crate::editor::command_registry::EncryptionCommand;
use crate::editor::core::Editor;
use crate::sync::{SyncClient, proto};

/// Read the account marker with a bounded wait, so a slow or unreachable server
/// never hangs the reader. `None` = not connected, timed out, or errored.
pub(super) fn marker_with_timeout() -> Option<proto::EncryptionState> {
  let client = SyncClient::from_config(&load_server_config())?;
  let (tx, rx) = channel();
  thread::spawn(move || {
    let _ = tx.send(client.get_encryption());
  });
  rx.recv_timeout(Duration::from_secs(10)).ok()?.ok()
}

/// After authenticating, reconcile local state with the account marker and, if
/// this device still needs the wizard, return the nudge lines. `None` means
/// nothing to prompt (already set up, or the account isn't encrypted).
pub(super) fn first_connect_nudge() -> Option<Vec<String>> {
  let state = marker_with_timeout()?;
  let cfg = load_encryption_config();
  if !state.enabled {
    // The account is no longer encrypted (disabled here or from the server).
    if cfg.resolve_key().is_some() {
      // We still hold the key: decrypt our documents back to plaintext in the
      // background, then forget it.
      super::encryption_setup::spawn_background_decrypt();
    } else if cfg.enabled {
      // Stale flag with no usable key: just drop it so we stop sealing.
      let _ = save_encryption_config(&EncryptionConfig::default(), true);
    }
    return None;
  }
  if cfg.resolve_key().is_some() {
    return None; // already set up on this device
  }
  Some(if state.salt.is_empty() {
    // Server-mandated but not yet initialized by any client — set up here.
    vec![
      "  ━━━ This account requires end-to-end encryption ━━━".to_string(),
      "  ".to_string(),
      "  No key has been created yet. Set one up on this device:".to_string(),
      "      :encryption setup".to_string(),
      "  ".to_string(),
      "  :q to dismiss".to_string(),
    ]
  } else {
    // Initialized elsewhere — paste the account key.
    vec![
      "  ━━━ This account uses end-to-end encryption ━━━".to_string(),
      "  ".to_string(),
      "  This device needs the account key before it can read or upload"
        .to_string(),
      "  encrypted documents. From your password manager, run:".to_string(),
      "      :encryption use <key>".to_string(),
      "  ".to_string(),
      "  :q to dismiss".to_string(),
    ]
  })
}

/// Lines shown when a command needs a connection but none is set up.
pub(super) fn not_connected_lines() -> Vec<String> {
  vec![
    "  Not connected to a sync server.".to_string(),
    "  Connect first: :connect <url> then :auth <username> <token>."
      .to_string(),
    "  :q to dismiss".to_string(),
  ]
}

impl Editor {
  /// `:encryption …` — inspect or change end-to-end encryption. No argument
  /// shows a state-aware status/next-step screen.
  pub fn handle_encryption_command(
    &mut self,
    action: EncryptionCommand,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let lines = match action {
      EncryptionCommand::Show => self.encryption_status_lines(),
      EncryptionCommand::Setup => self.encryption_setup(),
      EncryptionCommand::Use(key) => self.encryption_use(key),
      EncryptionCommand::Convert => self.encryption_convert(),
      EncryptionCommand::Disable => self.encryption_disable(),
      EncryptionCommand::Forget => encryption_forget(),
    };
    self.finish_server_command(lines);
    Ok(false)
  }

  fn encryption_status_lines(&self) -> Vec<String> {
    let cfg = load_encryption_config();
    let has_key = cfg.resolve_key().is_some();
    let mut lines =
      vec!["  ━━━ End-to-end encryption ━━━".to_string(), "  ".to_string()];
    match marker_with_timeout() {
      Some(state) if state.enabled && has_key => {
        lines.push("  Status: ON — this device is set up.".to_string());
        lines.push(
          "  Documents and notes are sealed on this device before they reach"
            .to_string(),
        );
        lines.push(
          "  the server, which only ever stores unreadable ciphertext."
            .to_string(),
        );
        lines.push("  ".to_string());
        lines.push(
          "  :encryption convert   seal documents uploaded before you turned \
           it on"
            .to_string(),
        );
      }
      Some(state) if state.enabled && state.salt.is_empty() => {
        lines.push(
          "  Status: encryption is REQUIRED for this account (set on the"
            .to_string(),
        );
        lines.push("  server), but no key has been created yet.".to_string());
        lines.push("  ".to_string());
        lines.push(
          "  :encryption setup   create the key on this device".to_string(),
        );
      }
      Some(state) if state.enabled => {
        lines.push(
          "  Status: this account uses encryption — but THIS device is not"
            .to_string(),
        );
        lines
          .push("  set up, so it cannot read or upload documents.".to_string());
        lines.push("  ".to_string());
        lines.push(
          "  :encryption use <key>   paste the account key from your password"
            .to_string(),
        );
        lines.push("  manager to finish setting this device up.".to_string());
      }
      Some(_) => {
        lines.push("  Status: OFF for this account.".to_string());
        lines.push(
          "  Your uploaded documents and notes are readable by the server."
            .to_string(),
        );
        lines.push("  ".to_string());
        lines.push(
          "  :encryption setup   turn on end-to-end encryption".to_string(),
        );
        lines.push("  Every device must then use the same key.".to_string());
      }
      None => {
        lines.extend(not_connected_lines());
        return lines;
      }
    }
    lines.push("  ".to_string());
    lines.push("  :q to dismiss".to_string());
    lines
  }
}

/// `:encryption forget` — clear this device's key and settings (leaving the
/// account's encryption, and every other device, untouched).
fn encryption_forget() -> Vec<String> {
  match save_encryption_config(&EncryptionConfig::default(), true) {
    Ok(()) => vec![
      "  Cleared this device's encryption key and settings.".to_string(),
      "  The account is unchanged — other devices keep working.".to_string(),
      "  Re-add the key later with :encryption use <key>.".to_string(),
      "  :q to dismiss".to_string(),
    ],
    Err(e) => {
      vec![format!("  Couldn't update config: {e}"), "  :q to dismiss".into()]
    }
  }
}
