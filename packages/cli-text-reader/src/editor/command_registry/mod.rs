mod completion;
mod types;

#[cfg(test)]
mod inline_tests;

pub(crate) use completion::{complete_command, top_level_commands};
pub(crate) use types::{
  AutoSyncAction, CommandCompletion, EncryptionCommand, RegisteredCommand,
  SyncModeCommand, TutorialCommand, classify_command, command_help_lines,
};
