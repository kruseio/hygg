//! `:syncmode`, `:server-progress` and `:local-progress` command handlers.
//! Split out from the server command module to keep each file within the
//! repository's per-file line budget; behaviour is unchanged.

use std::time::Duration;

use hygg_shared::sync::SyncMode;

use crate::config::load_server_config;
use crate::editor::command_registry::SyncModeCommand;
use crate::editor::core::{Editor, EditorMode};
use crate::sync::SyncClient;

impl Editor {
  /// `:syncmode …` — inspect or change how the current document syncs. No
  /// argument reports the mode; `full|metadata|off` clamps this device;
  /// `inherit` follows the account ceiling; `server <mode>` sets the
  /// account-wide ceiling for every device.
  pub fn handle_sync_mode_command(
    &mut self,
    action: SyncModeCommand,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(book_id) = self.book_id.clone() else {
      self.finish_server_command(vec![
        "  Sync mode applies to a document tracked in your library."
          .to_string(),
        "  This content has no synced identity.".to_string(),
        "  :q to dismiss".to_string(),
      ]);
      return Ok(false);
    };
    let lines = match action {
      SyncModeCommand::Show => self.sync_mode_status_lines(),
      SyncModeCommand::SetLocal(mode) => self.set_local_sync_mode(mode),
      SyncModeCommand::SetServer(mode) => {
        self.set_server_sync_mode(&book_id, mode)
      }
    };
    self.finish_server_command(lines);
    Ok(false)
  }

  fn sync_mode_status_lines(&self) -> Vec<String> {
    let entry = crate::library::latest_entry(self.document_hash);
    let local = entry
      .as_ref()
      .and_then(|e| e.local_sync_mode)
      .map(|m| m.to_string())
      .unwrap_or_else(|| "inherit".to_string());
    let server =
      entry.as_ref().and_then(|e| e.server_sync_mode).unwrap_or(SyncMode::Full);
    vec![
      format!("  Sync mode for this document: {}", self.sync_mode),
      format!("  This device: {local}"),
      format!("  Account ceiling: {server}"),
      "  ".to_string(),
      "  :syncmode full|metadata|off   set this device".to_string(),
      "  :syncmode inherit             follow the account ceiling".to_string(),
      "  :syncmode server <mode>       set every device".to_string(),
      "  :q to dismiss".to_string(),
    ]
  }

  /// Clamp this device's sync for the current document (`None` = inherit), then
  /// re-push the current state if sync is (still) enabled so the change lands
  /// promptly.
  fn set_local_sync_mode(&mut self, mode: Option<SyncMode>) -> Vec<String> {
    let updated = crate::library::update_entry(self.document_hash, |e| {
      e.local_sync_mode = mode;
    });
    self.sync_mode = updated
      .as_ref()
      .map(|e| e.effective_sync_mode())
      .unwrap_or_else(|| mode.unwrap_or(SyncMode::Full));
    if self.sync_mode.syncs_state() {
      let _ = self.queue_current_sync_state();
      if let Some(sync) = self.sync.as_ref() {
        sync.flush_now();
      }
    }
    let label =
      mode.map(|m| m.to_string()).unwrap_or_else(|| "inherit".to_string());
    vec![
      format!("  This device now syncs this document: {label}"),
      format!("  Effective mode: {}", self.sync_mode),
      "  :q to dismiss".to_string(),
    ]
  }

  /// Set the account-wide ceiling on the server (bounded so a slow server can't
  /// hang the reader), then mirror it locally and re-evaluate the effective
  /// mode.
  fn set_server_sync_mode(
    &mut self,
    book_id: &str,
    mode: SyncMode,
  ) -> Vec<String> {
    let Some(client) = SyncClient::from_config(&load_server_config()) else {
      return vec![
        "  Not connected. Use :connect <url> and :auth <username> <token>."
          .to_string(),
        "  :q to dismiss".to_string(),
      ];
    };
    let book_id_owned = book_id.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
      let _ = tx.send(client.set_book_sync_mode(&book_id_owned, mode));
    });
    match rx.recv_timeout(Duration::from_secs(8)) {
      Ok(Ok(())) => {
        let updated = crate::library::update_entry(self.document_hash, |e| {
          e.server_sync_mode = Some(mode);
        });
        self.sync_mode =
          updated.as_ref().map(|e| e.effective_sync_mode()).unwrap_or(mode);
        if self.sync_mode.syncs_state() {
          let _ = self.queue_current_sync_state();
          if let Some(sync) = self.sync.as_ref() {
            sync.flush_now();
          }
        }
        vec![
          format!("  Account-wide sync ceiling set to {mode}."),
          "  Every device clamps to this or lower.".to_string(),
          format!("  Effective here: {}", self.sync_mode),
          "  :q to dismiss".to_string(),
        ]
      }
      Ok(Err(e)) => vec![
        format!("  Couldn't update the server: {e}"),
        "  :q to dismiss".to_string(),
      ],
      Err(_) => vec![
        "  The server didn't respond in time.".to_string(),
        "  :q to dismiss".to_string(),
      ],
    }
  }

  /// `:server-progress` — jump to the latest server position if one is pending,
  /// otherwise request a fresh pull for next time.
  pub fn handle_server_progress_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    if self.pending_server_progress.is_some() {
      self.jump_to_server_progress();
      self.set_active_mode(EditorMode::Normal);
      self.editor_state.command_buffer.clear();
      self.editor_state.command_cursor_pos = 0;
      self.mark_dirty();
      return Ok(false);
    }
    if self.sync.is_none() {
      self.finish_server_command(vec![
        "  Not connected to a sync server.".to_string(),
        "  :q to dismiss".to_string(),
      ]);
      return Ok(false);
    }
    // No cached position: re-fetch the current server position (a full pull, so
    // it arrives even when unchanged since our delta cursor) and arm the
    // request so it jumps automatically on arrival — no second
    // `:server-progress` needed.
    if let Some(sync) = self.sync.as_ref() {
      sync.refetch_progress();
    }
    self.server_progress_jump_requested_at = Some(std::time::Instant::now());
    self.finish_server_command(vec![
      "  Checking the server for the latest position…".to_string(),
      "  :q to dismiss".to_string(),
    ]);
    Ok(false)
  }

  /// `:local-progress` — discard any pending server position and upload the
  /// current local position as the newest progress.
  pub fn handle_local_progress_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    if self.sync.is_none() {
      self.finish_server_command(vec![
        "  Not connected to a sync server.".to_string(),
        "  :q to dismiss".to_string(),
      ]);
      return Ok(false);
    }

    // Choosing to keep and push local progress opts the document into
    // auto-sync, so the enqueue gate passes and it keeps syncing afterwards.
    if self.book_id.is_some() {
      self.set_auto_sync_optin(true);
    }
    let overwrite_after =
      self.pending_server_progress.as_ref().map(|progress| progress.updated_at);
    self.pending_server_progress = None;
    self.server_progress_prompt = false;
    self.server_progress_scroll_at = None;
    self.server_progress_jump_requested_at = None;
    self.startup_progress_reconcile = false;

    let lines = if let Err(e) =
      self.queue_current_sync_state_newer_than(overwrite_after)
    {
      vec![
        format!("  Could not queue local progress: {e}"),
        "  :q to dismiss".to_string(),
      ]
    } else {
      if let Some(sync) = self.sync.as_ref() {
        sync.sync_now();
      }
      vec![
        "  Keeping local progress.".to_string(),
        "  Syncing it to the server now…".to_string(),
        "  :q to dismiss".to_string(),
      ]
    };
    self.finish_server_command(lines);
    Ok(false)
  }
}
