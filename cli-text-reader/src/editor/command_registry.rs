use super::speech::SpeakAction;

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

struct CommandSpec {
  name: &'static str,
  arguments: &'static [&'static str],
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

const COMMANDS: &[CommandSpec] = &[
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

pub(crate) fn complete_command(input: &str) -> CommandCompletion {
  if input.starts_with('!') {
    return CommandCompletion { replacement: None, suggestions: Vec::new() };
  }

  let ends_with_space = input.chars().last().is_some_and(char::is_whitespace);
  let mut words = input.split_whitespace();
  let Some(command) = words.next() else {
    return CommandCompletion {
      replacement: None,
      suggestions: top_level_commands(),
    };
  };
  let rest: Vec<&str> = words.collect();

  if rest.is_empty() && !ends_with_space {
    return complete_top_level(command);
  }

  complete_argument(input, command, rest.last().copied(), ends_with_space)
}

fn complete_top_level(prefix: &str) -> CommandCompletion {
  let matches: Vec<&str> = COMMANDS
    .iter()
    .map(|command| command.name)
    .filter(|command| command.starts_with(prefix))
    .collect();

  match matches.as_slice() {
    [command] => CommandCompletion {
      replacement: Some((*command).to_string()),
      suggestions: Vec::new(),
    },
    _ => CommandCompletion { replacement: None, suggestions: matches },
  }
}

fn complete_argument(
  input: &str,
  command: &str,
  current_arg: Option<&str>,
  ends_with_space: bool,
) -> CommandCompletion {
  let Some(spec) = COMMANDS.iter().find(|spec| spec.name == command) else {
    return CommandCompletion { replacement: None, suggestions: Vec::new() };
  };
  if spec.arguments.is_empty() {
    return CommandCompletion { replacement: None, suggestions: Vec::new() };
  }

  let prefix = if ends_with_space { "" } else { current_arg.unwrap_or("") };
  let matches: Vec<&str> = spec
    .arguments
    .iter()
    .copied()
    .filter(|argument| {
      argument.starts_with(prefix)
        || (*argument == "{n}" && prefix.chars().all(|c| c.is_ascii_digit()))
    })
    .collect();

  match matches.as_slice() {
    [argument] if *argument != "{n}" => {
      let replacement = if ends_with_space {
        format!("{input}{argument}")
      } else {
        replace_last_word(input, argument)
      };
      CommandCompletion {
        replacement: Some(replacement),
        suggestions: Vec::new(),
      }
    }
    _ => CommandCompletion { replacement: None, suggestions: matches },
  }
}

fn replace_last_word(input: &str, replacement: &str) -> String {
  let Some((start, _)) =
    input.char_indices().rev().find(|(_, c)| c.is_whitespace())
  else {
    return replacement.to_string();
  };
  format!("{}{}", &input[..=start], replacement)
}

pub(crate) fn top_level_commands() -> Vec<&'static str> {
  COMMANDS.iter().map(|command| command.name).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn classifies_voice_with_id() {
    assert_eq!(
      classify_command("voice af_bella"),
      RegisteredCommand::Voice("af_bella".to_string())
    );
    // Blend syntax is a single token and must pass through untouched.
    assert_eq!(
      classify_command("voice af_heart.6+am_michael.4"),
      RegisteredCommand::Voice("af_heart.6+am_michael.4".to_string())
    );
  }

  #[test]
  fn classifies_speed_and_rejects_non_numeric() {
    assert_eq!(classify_command("speed 1.25"), RegisteredCommand::Speed(1.25));
    assert_eq!(classify_command("speed 2"), RegisteredCommand::Speed(2.0));
    // Non-numeric speed is not a valid command.
    assert_eq!(classify_command("speed fast"), RegisteredCommand::Unknown);
  }

  #[test]
  fn voice_and_speed_without_args_are_not_setters() {
    // Bare `:voice` / `:speed` carry no value, so they are not setters.
    assert_eq!(classify_command("voice"), RegisteredCommand::Unknown);
    assert_eq!(classify_command("speed"), RegisteredCommand::Unknown);
  }

  #[test]
  fn voice_completes_known_ids() {
    let completion = complete_command("voice af_he");
    assert_eq!(completion.replacement.as_deref(), Some("voice af_heart"));
  }
}
