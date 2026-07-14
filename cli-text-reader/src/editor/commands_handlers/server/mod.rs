//! Server/sync commands. Phase 3a wires the connection state into
//! `~/.config/hygg/.env` (so it reloads next session) and confirms via a
//! notification overlay. The background sync engine that uses these settings
//! lands in a later increment; everything here is local and offline-safe.

use uuid::Uuid;

use super::super::core::{Editor, EditorMode, SnapshotReason};
use crate::config::{ServerConfig, load_server_config, save_server_config};

mod autosync;
mod syncmode;

impl Editor {
  /// `:connect <url>` — set the sync server URL (generating a stable device id
  /// on first connect) and persist it.
  pub fn handle_connect_command(
    &mut self,
    url: String,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let mut config = load_server_config();
    config.server_url = Some(url.clone());
    if config.device_id.is_none() {
      config.device_id = Some(Uuid::new_v4().to_string());
    }
    let lines = match save_server_config(&config) {
      Ok(()) => {
        self.apply_sync_config(&config);
        vec![
          format!("  Connected to {url}"),
          "  ".to_string(),
          "  Next: :auth <username> <token> to authenticate this device."
            .to_string(),
          "  Sync starts automatically after authentication.".to_string(),
          "  :q to dismiss".to_string(),
        ]
      }
      Err(e) => {
        vec![format!("  Failed to save: {e}"), "  :q to dismiss".into()]
      }
    };
    self.finish_server_command(lines);
    Ok(false)
  }

  /// `:disconnect` — clear the server URL and token (local data is untouched).
  pub fn handle_disconnect_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let mut config = load_server_config();
    config.server_url = None;
    config.api_token = None;
    let _ = save_server_config(&config);
    self.apply_sync_config(&config);
    self.finish_server_command(vec![
      "  Disconnected from sync server".to_string(),
      "  Your documents, progress and notes remain available offline."
        .to_string(),
      "  :q to dismiss".to_string(),
    ]);
    Ok(false)
  }

  /// `:auth <username> <token>` — store the account username and device API
  /// token. Both are required: the server checks the token against the named
  /// account, and binds the token to this machine on first sync.
  pub fn handle_auth_command(
    &mut self,
    username: String,
    token: String,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let mut config = load_server_config();
    if config.server_url.is_none() {
      self.finish_server_command(vec![
        "  Not connected. Run :connect <url> first.".to_string(),
        "  :q to dismiss".to_string(),
      ]);
      return Ok(false);
    }
    config.username = Some(username);
    config.api_token = Some(token);
    let _ = save_server_config(&config);
    self.apply_sync_config(&config);
    self.finish_server_command(vec![
      "  Authenticated. Sync starts automatically.".to_string(),
      "  This device is now locked to this machine.".to_string(),
      "  :q to dismiss".to_string(),
    ]);
    Ok(false)
  }

  /// Apply persisted sync config to this live editor session. The master switch
  /// (`sync_enabled`) decides whether the worker runs at all; the scope
  /// (`auto_sync`) mirrors onto the editor so the enqueue gate is current.
  /// `off` tears the worker down (fully serverless); connect/auth changes
  /// restart it with the latest URL/token when both are present.
  fn apply_sync_config(&mut self, config: &ServerConfig) {
    if let Some(mut sync) = self.sync.take() {
      sync.shutdown();
    }
    self.sync_policy = config.auto_sync;
    if config.sync_enabled {
      self.sync = crate::sync::SyncHandle::spawn(config);
      if self.sync.is_some() {
        self.queue_reconcile_sync_state(false);
        if let Some(sync) = self.sync.as_ref() {
          sync.flush_now();
        }
      }
    }
  }

  pub(crate) fn queue_current_sync_state(
    &mut self,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.enqueue_current_book_sync();
    self.save_progress_snapshot(SnapshotReason::Explicit)
  }

  pub(crate) fn queue_current_sync_state_newer_than(
    &mut self,
    newer_than: Option<i64>,
  ) -> Result<(), Box<dyn std::error::Error>> {
    self.queue_current_sync_state()?;
    let Some(newer_than) = newer_than else {
      return Ok(());
    };
    if self.total_lines == 0 {
      return Ok(());
    }
    if self.pdf_pending.is_some() && self.pdf_streaming.is_none() {
      return Ok(());
    }

    let current_line = self.offset + self.cursor_y;
    let (page, line_in_page) = match self.current_pdf_position() {
      Some((p, l)) => (Some(p), Some(l)),
      None => (None, None),
    };
    let updated_at =
      overwrite_progress_timestamp(self.sync_now_ms(), newer_than);
    self.enqueue_progress_sync_at(current_line, page, line_in_page, updated_at);
    Ok(())
  }

  pub(crate) fn finish_server_command(&mut self, lines: Vec<String>) {
    self.create_overlay("notification", lines);
    self.set_active_mode(EditorMode::Normal);
    self.editor_state.command_buffer.clear();
    self.editor_state.command_cursor_pos = 0;
    if let Some(buffer) = self.buffers.get_mut(self.active_buffer) {
      buffer.command_buffer.clear();
      buffer.command_cursor_pos = 0;
    }
    self.mark_dirty();
  }
}

fn overwrite_progress_timestamp(now: i64, newer_than: i64) -> i64 {
  now.max(newer_than.saturating_add(1))
}

#[cfg(test)]
mod tests {
  use super::overwrite_progress_timestamp;

  #[test]
  fn overwrite_progress_timestamp_beats_pending_server_timestamp() {
    assert_eq!(overwrite_progress_timestamp(1_000, 2_000), 2_001);
    assert_eq!(overwrite_progress_timestamp(3_000, 2_000), 3_000);
    assert_eq!(overwrite_progress_timestamp(10, i64::MAX), i64::MAX);
  }
}
