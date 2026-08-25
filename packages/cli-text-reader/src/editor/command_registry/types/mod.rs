use hygg_shared::sync::{AutoSyncPolicy, SyncMode};

use super::super::speech::SpeakAction;

mod classify;
mod help;

pub(crate) use classify::{COMMANDS, classify_command};
pub(crate) use help::command_help_lines;

#[derive(Debug, PartialEq)]
pub(crate) struct CommandCompletion {
  pub(crate) replacement: Option<String>,
  pub(crate) suggestions: Vec<&'static str>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum RegisteredCommand {
  About,
  AutoSync(AutoSyncAction),
  Back,
  Credits,
  Cursor,
  Encryption(EncryptionCommand),
  Help,
  Highlight,
  Home,
  LocalProgress,
  Next,
  NoHighlight,
  Note,
  NoTutorial,
  Ocr(bool),
  ServerAuth { username: String, token: String },
  ServerConnect(String),
  ServerDisconnect,
  ServerProgress,
  Sync,
  SyncMode(SyncModeCommand),
  Progress,
  Quit,
  Shell(String),
  Speak(SpeakAction),
  Voice(String),
  Speed(f32),
  ToggleHighlighter,
  Tutorial(TutorialCommand),
  Unknown,
}

#[derive(Debug, PartialEq)]
pub(crate) enum TutorialCommand {
  Default,
  Enabled(bool),
  Step(usize),
}

/// `:autosync` variants. `Show` reports the master switch, scope, and this
/// document's status; `Master` toggles the serverless kill switch; `Scope`
/// sets which documents auto-sync (all / books / manual); `OptIn` adds or
/// removes the current document from auto-sync.
#[derive(Debug, PartialEq)]
pub(crate) enum AutoSyncAction {
  Show,
  Master(bool),
  Scope(AutoSyncPolicy),
  OptIn(bool),
}

/// `:encryption` variants — the end-to-end encryption setup wizard.
/// `Show` reports the state and next step; `Setup` generates a new account key
/// and turns encryption on; `Use` adopts an existing account key on this
/// client (the new-device path); `Convert` re-uploads existing documents
/// encrypted; `Forget` clears this client's key without touching the account.
#[derive(Debug, PartialEq)]
pub(crate) enum EncryptionCommand {
  Show,
  Setup,
  Use(String),
  Convert,
  Disable,
  Forget,
}

/// `:syncmode` variants. `Show` reports the current mode; `SetLocal` clamps
/// this device only (`None` = inherit the server ceiling); `SetServer` moves
/// the account-wide ceiling on the server.
#[derive(Debug, PartialEq)]
pub(crate) enum SyncModeCommand {
  Show,
  SetLocal(Option<SyncMode>),
  SetServer(SyncMode),
}
