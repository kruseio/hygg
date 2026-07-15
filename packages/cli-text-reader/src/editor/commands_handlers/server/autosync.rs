//! `:autosync` and `:sync` command handlers. Split out from the server command
//! module to keep each file within the repository's per-file line budget;
//! behaviour is unchanged.

use hygg_shared::sync::AutoSyncPolicy;

use crate::config::{load_server_config, save_server_config};
use crate::editor::command_registry::AutoSyncAction;
use crate::editor::core::Editor;

impl Editor {
  /// `:autosync …` — inspect or change what syncs automatically. No argument
  /// reports status; `on|off` is the master serverless kill switch;
  /// `all|books|manual` sets which documents auto-sync; `add|remove` opts the
  /// current document in or out.
  pub fn handle_autosync_command(
    &mut self,
    action: AutoSyncAction,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let lines = match action {
      AutoSyncAction::Show => self.autosync_status_lines(),
      AutoSyncAction::Master(enabled) => self.set_sync_enabled(enabled),
      AutoSyncAction::Scope(policy) => self.set_auto_sync_scope(policy),
      AutoSyncAction::OptIn(opt_in) => self.set_doc_auto_sync_optin(opt_in),
    };
    self.finish_server_command(lines);
    Ok(false)
  }

  fn autosync_status_lines(&self) -> Vec<String> {
    let config = load_server_config();
    let doc = if !config.sync_enabled {
      "no (sync is off)"
    } else if self.doc_auto_syncs() {
      "yes"
    } else {
      "no (not opted in)"
    };
    let scope = match config.auto_sync {
      AutoSyncPolicy::All => "all documents",
      AutoSyncPolicy::Books => "book-like documents + opted-in",
      AutoSyncPolicy::Manual => "only opted-in documents",
    };
    vec![
      format!(
        "  Sync: {}",
        if config.sync_enabled { "on" } else { "off (serverless)" }
      ),
      format!("  Auto-sync scope: {} ({scope})", config.auto_sync),
      format!("  This document auto-syncs: {doc}"),
      "  ".to_string(),
      "  :autosync on|off            master switch".to_string(),
      "  :autosync all|books|manual  which documents auto-sync".to_string(),
      "  :autosync add|remove        auto-sync this document".to_string(),
      "  :q to dismiss".to_string(),
    ]
  }

  /// Master switch: `off` tears the engine down (fully serverless); `on`
  /// restarts it with the current URL/token.
  fn set_sync_enabled(&mut self, enabled: bool) -> Vec<String> {
    let mut config = load_server_config();
    config.sync_enabled = enabled;
    let _ = save_server_config(&config);
    self.apply_sync_config(&config);
    if enabled {
      vec![
        "  Sync on.".to_string(),
        format!("  Auto-syncing {}.", scope_phrase(config.auto_sync)),
        "  :q to dismiss".to_string(),
      ]
    } else {
      vec![
        "  Sync off — this device is now fully serverless.".to_string(),
        "  Your documents, progress and notes remain available offline."
          .to_string(),
        "  :q to dismiss".to_string(),
      ]
    }
  }

  /// Change which documents auto-sync, persist it, and re-push the current
  /// document if it now qualifies.
  fn set_auto_sync_scope(&mut self, policy: AutoSyncPolicy) -> Vec<String> {
    let mut config = load_server_config();
    config.auto_sync = policy;
    let _ = save_server_config(&config);
    self.sync_policy = policy;
    if self.sync.is_some() && self.doc_auto_syncs() {
      let _ = self.queue_current_sync_state();
      if let Some(sync) = self.sync.as_ref() {
        sync.flush_now();
      }
    }
    vec![
      format!(
        "  Auto-sync scope: {policy} — syncing {}.",
        scope_phrase(policy)
      ),
      "  :q to dismiss".to_string(),
    ]
  }

  /// Add or remove the current document from auto-sync, persisting the opt-in
  /// on its library entry. Opting in re-pushes it immediately.
  fn set_doc_auto_sync_optin(&mut self, opt_in: bool) -> Vec<String> {
    if self.book_id.is_none() {
      return vec![
        "  Auto-sync opt-in applies to a document in your library.".to_string(),
        "  This content has no synced identity.".to_string(),
        "  :q to dismiss".to_string(),
      ];
    }
    self.set_auto_sync_optin(opt_in);
    if opt_in {
      if self.sync.is_some() {
        let _ = self.queue_current_sync_state();
        if let Some(sync) = self.sync.as_ref() {
          sync.flush_now();
        }
      }
      vec![
        "  This document now auto-syncs.".to_string(),
        "  :q to dismiss".to_string(),
      ]
    } else {
      vec![
        "  This document no longer auto-syncs.".to_string(),
        "  Its data stays on this device unless the scope covers it."
          .to_string(),
        "  :q to dismiss".to_string(),
      ]
    }
  }

  /// Persist this document's auto-sync opt-in on its library entry and mirror
  /// it onto the live editor so the enqueue gate sees it immediately.
  pub(crate) fn set_auto_sync_optin(&mut self, opt_in: bool) {
    self.auto_sync_optin = opt_in;
    crate::library::update_entry(self.document_hash, |e| {
      e.auto_sync_optin = opt_in;
    });
  }

  /// `:sync` — force the background engine to flush queued changes and pull
  /// remote updates now. Syncing a document explicitly opts it into auto-sync,
  /// so it keeps syncing afterwards (the manual way to add a report or note).
  pub fn handle_sync_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    let lines = match self.sync.as_ref() {
      Some(_) => {
        if self.book_id.is_some() {
          self.set_auto_sync_optin(true);
        }
        if let Err(e) = self.queue_current_sync_state() {
          vec![
            format!("  Could not queue sync: {e}"),
            "  :q to dismiss".to_string(),
          ]
        } else {
          if let Some(sync) = self.sync.as_ref() {
            sync.sync_now();
          }
          vec!["  Syncing…".to_string(), "  :q to dismiss".to_string()]
        }
      }
      None => vec![
        "  Not connected. Use :connect <url> and :auth <username> <token>."
          .to_string(),
        "  :q to dismiss".to_string(),
      ],
    };
    self.finish_server_command(lines);
    Ok(false)
  }
}

/// Human-readable phrase for what a scope auto-syncs, for confirmations.
fn scope_phrase(policy: AutoSyncPolicy) -> &'static str {
  match policy {
    AutoSyncPolicy::All => "every document",
    AutoSyncPolicy::Books => "book-like documents and opted-in ones",
    AutoSyncPolicy::Manual => "only opted-in documents",
  }
}
