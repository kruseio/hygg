use std::collections::HashMap;

#[derive(Debug, Clone)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub struct ParsedCommand {
  pub command: String,
  pub flags: Vec<String>,
  pub args: Vec<String>,
}

impl ParsedCommand {
  pub fn parse(cmd: &str) -> Option<Self> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
      return None;
    }

    let command = parts[0].to_string();
    let mut flags = Vec::new();
    let mut args = Vec::new();

    let mut i = 1;
    while i < parts.len() {
      let part = parts[i];
      if part.starts_with('-') {
        flags.push(part.to_string());
      } else {
        args.push(part.to_string());
      }
      i += 1;
    }

    Some(ParsedCommand { command, flags, args })
  }
}

// Translate Unix commands to PowerShell equivalents
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub fn translate_command_for_windows(cmd: &str) -> String {
  let parsed = match ParsedCommand::parse(cmd) {
    Some(p) => p,
    None => return cmd.to_string(),
  };

  match parsed.command.as_str() {
    "ls" => translate_ls(&parsed),
    "cat" => translate_cat(&parsed),
    "grep" => translate_grep(&parsed),
    "pwd" => "Get-Location".to_string(),
    "head" => translate_head(&parsed),
    "tail" => translate_tail(&parsed),
    _ => cmd.to_string(), // Return unchanged if not a known command
  }
}

fn translate_ls(parsed: &ParsedCommand) -> String {
  let mut ps_cmd = String::from("Get-ChildItem");

  // Handle flags
  let mut format_table = false;
  let mut show_hidden = false;

  for flag in &parsed.flags {
    match flag.as_str() {
      "-l" | "-la" | "-al" => {
        format_table = true;
        if flag.contains('a') {
          show_hidden = true;
        }
      }
      "-a" => show_hidden = true,
      _ => {}
    }
  }

  // Add path arguments
  if !parsed.args.is_empty() {
    ps_cmd.push_str(" -Path ");
    ps_cmd.push_str(&parsed.args.join(", "));
  }

  if show_hidden {
    ps_cmd.push_str(" -Force");
  }

  if format_table {
    // User explicitly wants detailed format
    ps_cmd.push_str(" | Format-Table Mode, LastWriteTime, Length, Name");
  } else {
    // Default behavior: one file per line, just like Linux ls
    ps_cmd.push_str(" | Select-Object -ExpandProperty Name");
  }

  ps_cmd
}

fn translate_cat(parsed: &ParsedCommand) -> String {
  let mut ps_cmd = String::from("Get-Content");

  // Add file arguments
  if !parsed.args.is_empty() {
    ps_cmd.push(' ');
    ps_cmd.push_str(&parsed.args.join(" "));
  }

  // Handle -n flag (number lines)
  if parsed.flags.contains(&"-n".to_string()) {
    ps_cmd = format!(
      "{ps_cmd} | Select-Object @{{Name='LineNumber';Expression={{$_.ReadCount}}}}, @{{Name='Line';Expression={{$_}}}}"
    );
  }

  ps_cmd
}

fn translate_grep(parsed: &ParsedCommand) -> String {
  if parsed.args.len() < 2 {
    return "Select-String".to_string();
  }

  let pattern = &parsed.args[0];
  let file = &parsed.args[1];

  let mut ps_cmd =
    format!("Select-String -Pattern \"{pattern}\" -Path \"{file}\"");

  // Handle flags
  let mut show_line_numbers = false;

  for flag in &parsed.flags {
    match flag.as_str() {
      "-i" => ps_cmd.push_str(" -CaseSensitive:$false"),
      "-n" => show_line_numbers = true,
      "-v" => ps_cmd.push_str(" -NotMatch"),
      _ => {}
    }
  }

  if show_line_numbers {
    ps_cmd.push_str(" | Format-Table LineNumber, Line");
  }

  ps_cmd
}

fn translate_head(parsed: &ParsedCommand) -> String {
  let mut ps_cmd = String::from("Get-Content");

  // Add file arguments
  if !parsed.args.is_empty() {
    ps_cmd.push(' ');
    ps_cmd.push_str(&parsed.args[0]);
  }

  // Default to 10 lines
  let mut line_count = 10;

  // Check for -n flag (can be -n10 or -n 10)
  for (i, flag) in parsed.flags.iter().enumerate() {
    if flag == "-n" && i + 1 < parsed.args.len() {
      // Next argument might be the number
      if let Ok(n) = parsed.args[0].parse::<usize>() {
        line_count = n;
      }
    } else if flag.starts_with("-n") && flag.len() > 2 {
      // Format like -n10
      if let Ok(n) = flag[2..].parse::<usize>() {
        line_count = n;
      }
    }
  }

  ps_cmd.push_str(&format!(" -First {line_count}"));
  ps_cmd
}

fn translate_tail(parsed: &ParsedCommand) -> String {
  let mut ps_cmd = String::from("Get-Content");

  // Add file arguments
  if !parsed.args.is_empty() {
    ps_cmd.push(' ');
    ps_cmd.push_str(&parsed.args[0]);
  }

  // Default to 10 lines
  let mut line_count = 10;

  // Check for -n flag (can be -n10 or -n 10)
  for (i, flag) in parsed.flags.iter().enumerate() {
    if flag == "-n" && i + 1 < parsed.args.len() {
      // Next argument might be the number
      if let Ok(n) = parsed.args[0].parse::<usize>() {
        line_count = n;
      }
    } else if flag.starts_with("-n") && flag.len() > 2 {
      // Format like -n10
      if let Ok(n) = flag[2..].parse::<usize>() {
        line_count = n;
      }
    }
  }

  ps_cmd.push_str(&format!(" -Tail {line_count}"));
  ps_cmd
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ls_translation() {
    assert_eq!(
      translate_command_for_windows("ls"),
      "Get-ChildItem | Select-Object -ExpandProperty Name"
    );
    assert_eq!(
      translate_command_for_windows("ls -la"),
      "Get-ChildItem -Force | Format-Table Mode, LastWriteTime, Length, Name"
    );
    assert_eq!(
      translate_command_for_windows("ls -l /tmp"),
      "Get-ChildItem -Path /tmp | Format-Table Mode, LastWriteTime, Length, Name"
    );
    assert_eq!(
      translate_command_for_windows("ls /tmp"),
      "Get-ChildItem -Path /tmp | Select-Object -ExpandProperty Name"
    );
    assert_eq!(
      translate_command_for_windows("ls -a"),
      "Get-ChildItem -Force | Select-Object -ExpandProperty Name"
    );
  }

  #[test]
  fn test_cat_translation() {
    assert_eq!(
      translate_command_for_windows("cat file.txt"),
      "Get-Content file.txt"
    );
    assert_eq!(
      translate_command_for_windows("cat -n file.txt"),
      "Get-Content file.txt | Select-Object @{Name='LineNumber';Expression={$_.ReadCount}}, @{Name='Line';Expression={$_}}"
    );
  }

  #[test]
  fn test_grep_translation() {
    assert_eq!(
      translate_command_for_windows("grep pattern file.txt"),
      "Select-String -Pattern \"pattern\" -Path \"file.txt\""
    );
    assert_eq!(
      translate_command_for_windows("grep -i pattern file.txt"),
      "Select-String -Pattern \"pattern\" -Path \"file.txt\" -CaseSensitive:$false"
    );
    assert_eq!(
      translate_command_for_windows("grep -n pattern file.txt"),
      "Select-String -Pattern \"pattern\" -Path \"file.txt\" | Format-Table LineNumber, Line"
    );
  }
}
