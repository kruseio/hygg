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

/// `:syncmode` variants. `Show` reports the current mode; `SetLocal` clamps
/// this device only (`None` = inherit the server ceiling); `SetServer` moves
/// the account-wide ceiling on the server.
#[derive(Debug, PartialEq)]
pub(crate) enum SyncModeCommand {
  Show,
  SetLocal(Option<SyncMode>),
  SetServer(SyncMode),
}
