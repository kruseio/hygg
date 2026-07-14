//! The command table and the string-to-`RegisteredCommand` classifier. Split
//! out from `types` to keep each file within the repository's per-file line
//! budget.

use hygg_shared::sync::{AutoSyncPolicy, SyncMode};

use super::{
  AutoSyncAction, RegisteredCommand, SyncModeCommand, TutorialCommand,
};
use crate::editor::speech::SpeakAction;

pub(crate) struct CommandSpec {
  pub(crate) name: &'static str,
  pub(crate) arguments: &'static [&'static str],
}

const OCR_ARGS: &[&str] = &["on", "off"];
const AUTOSYNC_ARGS: &[&str] =
  &["on", "off", "all", "books", "manual", "add", "remove"];
const CONNECT_ARGS: &[&str] = &["{url}"];
const AUTH_ARGS: &[&str] = &["{username}", "{token}"];
const SPEAK_ARGS: &[&str] = &["stop"];
// Curated high-quality English voices for tab-completion; any Kokoro voice id
// (or a blend like "af_heart.6+am_michael.4") is still accepted when typed.
const VOICE_ARGS: &[&str] = &[
  "af_heart",
  "af_bella",
  "af_nicole",
  "af_sarah",
  "af_aoede",
  "af_kore",
  "am_michael",
  "am_fenrir",
  "am_puck",
  "bf_emma",
  "bm_george",
];
const SPEED_ARGS: &[&str] = &["{n}"];
const TUTORIAL_ARGS: &[&str] = &["on", "off", "{n}"];
const SYNCMODE_ARGS: &[&str] =
  &["full", "metadata", "off", "inherit", "server"];

pub(crate) const COMMANDS: &[CommandSpec] = &[
  CommandSpec { name: "about", arguments: &[] },
  CommandSpec { name: "auth", arguments: AUTH_ARGS },
  CommandSpec { name: "author", arguments: &[] },
  CommandSpec { name: "autosync", arguments: AUTOSYNC_ARGS },
  CommandSpec { name: "back", arguments: &[] },
  CommandSpec { name: "c", arguments: &[] },
  CommandSpec { name: "commands", arguments: &[] },
  CommandSpec { name: "connect", arguments: CONNECT_ARGS },
  CommandSpec { name: "continue", arguments: &[] },
  CommandSpec { name: "credits", arguments: &[] },
  CommandSpec { name: "disconnect", arguments: &[] },
  CommandSpec { name: "cursor", arguments: &[] },
  CommandSpec { name: "exit", arguments: &[] },
  CommandSpec { name: "h", arguments: &[] },
  CommandSpec { name: "help", arguments: &[] },
  CommandSpec { name: "home", arguments: &[] },
  CommandSpec { name: "Rex", arguments: &[] },
  CommandSpec { name: "!", arguments: &[] },
  CommandSpec { name: "local-progress", arguments: &[] },
  CommandSpec { name: "next", arguments: &[] },
  CommandSpec { name: "nohl", arguments: &[] },
  CommandSpec { name: "nohlsearch", arguments: &[] },
  CommandSpec { name: "note", arguments: &[] },
  CommandSpec { name: "notutorial", arguments: &[] },
  CommandSpec { name: "ocr", arguments: OCR_ARGS },
  CommandSpec { name: "p", arguments: &[] },
  CommandSpec { name: "prev", arguments: &[] },
  CommandSpec { name: "previous", arguments: &[] },
  CommandSpec { name: "q", arguments: &[] },
  CommandSpec { name: "q!", arguments: &[] },
  CommandSpec { name: "quit", arguments: &[] },
  CommandSpec { name: "server-progress", arguments: &[] },
  CommandSpec { name: "speak", arguments: SPEAK_ARGS },
  CommandSpec { name: "sync", arguments: &[] },
  CommandSpec { name: "syncmode", arguments: SYNCMODE_ARGS },
  CommandSpec { name: "speed", arguments: SPEED_ARGS },
  CommandSpec { name: "tutorial", arguments: TUTORIAL_ARGS },
  CommandSpec { name: "voice", arguments: VOICE_ARGS },
  CommandSpec { name: "z", arguments: &[] },
];

pub(crate) fn classify_command(input: &str) -> RegisteredCommand {
  let input = input.trim();
  if matches!(input, "q" | "q!" | "quit" | "exit") {
    return RegisteredCommand::Quit;
  }
  if let Some(shell_command) = input.strip_prefix('!') {
    return RegisteredCommand::Shell(shell_command.to_string());
  }

  let mut parts = input.split_whitespace();
  let Some(command) = parts.next() else {
    return RegisteredCommand::Unknown;
  };
  let args: Vec<&str> = parts.collect();

  match (command, args.as_slice()) {
    ("p", []) => RegisteredCommand::Progress,
    ("cursor" | "c", []) => RegisteredCommand::Cursor,
    ("help" | "commands", []) => RegisteredCommand::Help,
    ("home" | "Rex", []) => RegisteredCommand::Home,
    ("connect", [url]) => RegisteredCommand::ServerConnect((*url).to_string()),
    ("disconnect", []) => RegisteredCommand::ServerDisconnect,
    ("auth", [username, token]) => RegisteredCommand::ServerAuth {
      username: (*username).to_string(),
      token: (*token).to_string(),
    },
    ("autosync", []) => RegisteredCommand::AutoSync(AutoSyncAction::Show),
    ("autosync", ["on"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::Master(true))
    }
    ("autosync", ["off"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::Master(false))
    }
    ("autosync", ["all"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::Scope(AutoSyncPolicy::All))
    }
    ("autosync", ["books"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::Scope(AutoSyncPolicy::Books))
    }
    ("autosync", ["manual"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::Scope(AutoSyncPolicy::Manual))
    }
    ("autosync", ["add"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::OptIn(true))
    }
    ("autosync", ["remove"]) => {
      RegisteredCommand::AutoSync(AutoSyncAction::OptIn(false))
    }
    ("sync", []) => RegisteredCommand::Sync,
    ("server-progress", []) => RegisteredCommand::ServerProgress,
    ("local-progress", []) => RegisteredCommand::LocalProgress,
    ("syncmode", []) => RegisteredCommand::SyncMode(SyncModeCommand::Show),
    ("syncmode", ["server", mode]) => match mode.parse::<SyncMode>() {
      Ok(mode) => RegisteredCommand::SyncMode(SyncModeCommand::SetServer(mode)),
      Err(()) => RegisteredCommand::Unknown,
    },
    ("syncmode", [mode]) => match mode.trim().to_ascii_lowercase().as_str() {
      "inherit" | "default" | "clear" => {
        RegisteredCommand::SyncMode(SyncModeCommand::SetLocal(None))
      }
      token => match token.parse::<SyncMode>() {
        Ok(mode) => {
          RegisteredCommand::SyncMode(SyncModeCommand::SetLocal(Some(mode)))
        }
        Err(()) => RegisteredCommand::Unknown,
      },
    },
    ("notutorial", []) => RegisteredCommand::NoTutorial,
    ("tutorial", []) => RegisteredCommand::Tutorial(TutorialCommand::Default),
    ("tutorial", ["on"]) => {
      RegisteredCommand::Tutorial(TutorialCommand::Enabled(true))
    }
    ("tutorial", ["off"]) => {
      RegisteredCommand::Tutorial(TutorialCommand::Enabled(false))
    }
    ("tutorial", [step]) => step
      .parse::<usize>()
      .map(|step| RegisteredCommand::Tutorial(TutorialCommand::Step(step)))
      .unwrap_or(RegisteredCommand::Tutorial(TutorialCommand::Default)),
    ("tutorial", _) => RegisteredCommand::Tutorial(TutorialCommand::Default),
    ("next" | "continue", []) => RegisteredCommand::Next,
    ("note", []) => RegisteredCommand::Note,
    ("back" | "prev" | "previous", []) => RegisteredCommand::Back,
    ("h", []) => RegisteredCommand::Highlight,
    ("nohl" | "nohlsearch", []) => RegisteredCommand::NoHighlight,
    ("credits" | "author", []) => RegisteredCommand::Credits,
    ("about", []) => RegisteredCommand::About,
    ("ocr", ["on"]) => RegisteredCommand::Ocr(true),
    ("ocr", ["off"]) => RegisteredCommand::Ocr(false),
    ("speak", []) => RegisteredCommand::Speak(SpeakAction::Start),
    ("speak", ["stop"]) => RegisteredCommand::Speak(SpeakAction::Stop),
    ("voice", [id]) => RegisteredCommand::Voice((*id).to_string()),
    ("speed", [value]) => value
      .parse::<f32>()
      .map(RegisteredCommand::Speed)
      .unwrap_or(RegisteredCommand::Unknown),
    ("z", []) => RegisteredCommand::ToggleHighlighter,
    _ => RegisteredCommand::Unknown,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use hygg_shared::sync::AutoSyncPolicy;

  #[test]
  fn autosync_command_variants_classify() {
    use AutoSyncAction::*;
    let cases = [
      ("autosync", Show),
      ("autosync on", Master(true)),
      ("autosync off", Master(false)),
      ("autosync all", Scope(AutoSyncPolicy::All)),
      ("autosync books", Scope(AutoSyncPolicy::Books)),
      ("autosync manual", Scope(AutoSyncPolicy::Manual)),
      ("autosync add", OptIn(true)),
      ("autosync remove", OptIn(false)),
    ];
    for (input, expected) in cases {
      assert_eq!(
        classify_command(input),
        RegisteredCommand::AutoSync(expected),
        "input: :{input}"
      );
    }
  }

  #[test]
  fn unknown_autosync_argument_is_unknown() {
    assert_eq!(classify_command("autosync wat"), RegisteredCommand::Unknown);
  }
}
