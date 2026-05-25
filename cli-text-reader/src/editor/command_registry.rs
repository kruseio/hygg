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
  CommandSpec { name: "tutorial", arguments: TUTORIAL_ARGS },
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
