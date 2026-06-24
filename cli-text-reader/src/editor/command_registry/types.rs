use super::super::speech::SpeakAction;

#[derive(Debug, PartialEq)]
pub(crate) struct CommandCompletion {
  pub(crate) replacement: Option<String>,
  pub(crate) suggestions: Vec<&'static str>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum RegisteredCommand {
  About,
  Back,
  Credits,
  Cursor,
  Help,
  Highlight,
  Next,
  NoHighlight,
  NoTutorial,
  Ocr(bool),
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

pub(crate) struct CommandSpec {
  pub(crate) name: &'static str,
  pub(crate) arguments: &'static [&'static str],
}

const OCR_ARGS: &[&str] = &["on", "off"];
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

pub(crate) const COMMANDS: &[CommandSpec] = &[
  CommandSpec { name: "about", arguments: &[] },
  CommandSpec { name: "author", arguments: &[] },
  CommandSpec { name: "back", arguments: &[] },
  CommandSpec { name: "c", arguments: &[] },
  CommandSpec { name: "commands", arguments: &[] },
  CommandSpec { name: "continue", arguments: &[] },
  CommandSpec { name: "credits", arguments: &[] },
  CommandSpec { name: "cursor", arguments: &[] },
  CommandSpec { name: "exit", arguments: &[] },
  CommandSpec { name: "h", arguments: &[] },
  CommandSpec { name: "help", arguments: &[] },
  CommandSpec { name: "!", arguments: &[] },
  CommandSpec { name: "next", arguments: &[] },
  CommandSpec { name: "nohl", arguments: &[] },
  CommandSpec { name: "nohlsearch", arguments: &[] },
  CommandSpec { name: "notutorial", arguments: &[] },
  CommandSpec { name: "ocr", arguments: OCR_ARGS },
  CommandSpec { name: "p", arguments: &[] },
  CommandSpec { name: "prev", arguments: &[] },
  CommandSpec { name: "previous", arguments: &[] },
  CommandSpec { name: "q", arguments: &[] },
  CommandSpec { name: "q!", arguments: &[] },
  CommandSpec { name: "quit", arguments: &[] },
  CommandSpec { name: "speak", arguments: SPEAK_ARGS },
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

pub(crate) fn command_help_lines() -> Vec<String> {
  vec![
    "    :q, :q!, :quit, :exit  Quit".to_string(),
    "    :help, :commands       Show this help".to_string(),
    "    :tutorial              Start interactive tutorial".to_string(),
    "    :tutorial {n}          Jump to tutorial step n".to_string(),
    "    :tutorial on           Enable tutorial for next launch".to_string(),
    "    :tutorial off          Disable tutorial (same as :notutorial)"
      .to_string(),
    "    :notutorial            Permanently disable tutorial".to_string(),
    "    :next, :continue       Next tutorial step (when completed)"
      .to_string(),
    "    :back, :prev, :previous Previous tutorial step".to_string(),
    "    :z                     Toggle line highlighter".to_string(),
    "    :p                     Toggle progress display".to_string(),
    "    :ocr on, :ocr off      Toggle PDF OCR for this PDF and future launches"
      .to_string(),
    "    :speak, :speak stop    Narrate from the cursor (any key stops)"
      .to_string(),
    "    :voice <id>            Narration voice (e.g. af_heart, am_michael)"
      .to_string(),
    "    :speed <n>             Narration speed, 0.5-2.0 (e.g. 1.25)"
      .to_string(),
    "    :cursor, :c            Toggle cursor visibility".to_string(),
    "    :h                     Highlight selected text (in visual mode)"
      .to_string(),
    "    :nohl, :nohlsearch     Clear search highlighting".to_string(),
    "    :credits, :author      Show credits".to_string(),
    "    :about                 Show about information".to_string(),
    "    :!{cmd}                Execute shell command (opens in split view)"
      .to_string(),
  ]
}
