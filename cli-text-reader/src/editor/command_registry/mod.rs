mod completion;
mod types;

#[cfg(test)]
mod inline_tests;

pub(crate) use completion::{complete_command, top_level_commands};
pub(crate) use types::{
  CommandCompletion, RegisteredCommand, TutorialCommand, classify_command,
  command_help_lines,
};
