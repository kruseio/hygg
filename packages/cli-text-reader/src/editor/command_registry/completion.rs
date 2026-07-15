use super::types::{COMMANDS, CommandCompletion};

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
