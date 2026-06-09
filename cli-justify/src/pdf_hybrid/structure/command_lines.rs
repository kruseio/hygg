use crate::text_utils::{char_len, leading_whitespace};

pub(crate) fn code_line_continues(trimmed: &str) -> bool {
  trimmed.ends_with('\\')
    || trimmed.ends_with('|')
    || trimmed.ends_with("&&")
    || trimmed.ends_with("||")
    || trimmed.contains("<<")
}

pub(crate) fn looks_like_code_continuation_line(
  line: &str,
  base_indent_width: usize,
) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  let line_indent_width = char_len(leading_whitespace(line));
  line_indent_width > base_indent_width
    || trimmed.starts_with("&&")
    || trimmed.starts_with("||")
    || trimmed.starts_with('|')
    || trimmed == "EOF"
}

fn looks_like_env_assignment(token: &str) -> bool {
  let Some((name, value)) = token.split_once('=') else {
    return false;
  };
  !name.is_empty()
    && !value.is_empty()
    && name
      .chars()
      .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn strip_shell_wrappers(mut trimmed: &str) -> &str {
  while let Some((token, rest)) = trimmed.split_once(' ') {
    if token == "sudo" || token == "env" || token == "time" {
      trimmed = rest.trim_start();
      continue;
    }
    if looks_like_env_assignment(token) {
      trimmed = rest.trim_start();
      continue;
    }
    break;
  }
  trimmed
}

fn looks_like_path_arg(arg: &str) -> bool {
  arg == "."
    || arg == ".."
    || arg.starts_with(['/', '~'])
    || arg.starts_with("./")
    || arg.starts_with("../")
    || arg.contains('/')
    || arg.contains('\\')
    || arg.contains('.')
}

pub(crate) fn looks_like_shell_command_line(trimmed: &str) -> bool {
  let candidate = strip_shell_wrappers(trimmed);
  if !candidate.is_empty()
    && candidate.split_whitespace().all(looks_like_env_assignment)
  {
    return true;
  }

  let mut words = candidate.split_whitespace();
  let Some(cmd) = words.next() else {
    return false;
  };
  let Some(arg1) = words.next() else {
    return false;
  };

  if cmd.starts_with("./") || cmd.starts_with("../") {
    return true;
  }

  match cmd {
    "apt" | "apt-get" => matches!(
      arg1,
      "install" | "remove" | "update" | "upgrade" | "purge" | "search"
    ),
    "apk" | "brew" | "dnf" | "pacman" | "yum" => matches!(
      arg1,
      "add" | "install" | "remove" | "update" | "upgrade" | "search"
    ),
    "cargo" => matches!(
      arg1,
      "install" | "build" | "test" | "run" | "check" | "clippy" | "fmt"
    ),
    "git" => matches!(
      arg1,
      "add"
        | "branch"
        | "checkout"
        | "clone"
        | "commit"
        | "config"
        | "diff"
        | "fetch"
        | "init"
        | "log"
        | "merge"
        | "pull"
        | "push"
        | "rebase"
        | "remote"
        | "status"
        | "tag"
    ),
    "gh" => matches!(arg1, "auth" | "issue" | "pr" | "repo" | "run"),
    "docker" => matches!(
      arg1,
      "build" | "compose" | "exec" | "pull" | "run" | "start" | "stop"
    ),
    "kubectl" => matches!(
      arg1,
      "apply" | "create" | "delete" | "describe" | "get" | "logs"
    ),
    "npm" | "pnpm" | "yarn" => {
      matches!(arg1, "add" | "build" | "install" | "run" | "test" | "upgrade")
    }
    "pip" | "pip3" => matches!(arg1, "install" | "uninstall"),
    "python" | "python3" | "node" | "npx" => !arg1.is_empty(),
    "cat" | "curl" | "scp" | "ssh" | "wget" => !arg1.is_empty(),
    "cd" => looks_like_path_arg(arg1),
    "cmake" => {
      matches!(arg1, "--build" | "--install") || looks_like_path_arg(arg1)
    }
    "make" => matches!(
      arg1,
      "all" | "build" | "check" | "clean" | "configure" | "install" | "test"
    ),
    "mkdir" => arg1.starts_with('-') || looks_like_path_arg(arg1),
    "tar" => arg1.starts_with('-'),
    "hygg" => true,
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::looks_like_shell_command_line;

  #[test]
  fn recognises_unprompted_shell_commands() {
    assert!(looks_like_shell_command_line(
      "sudo apt install espeak-ng cmake pkgconf"
    ));
    assert!(looks_like_shell_command_line("tar -zxf git-2.8.0.tar.gz"));
    assert!(looks_like_shell_command_line("./configure --prefix=/usr"));
    assert!(looks_like_shell_command_line("make all doc info"));
  }

  #[test]
  fn rejects_prose_that_starts_with_command_words() {
    assert!(!looks_like_shell_command_line(
      "make sure the repository has a clean working tree"
    ));
    assert!(!looks_like_shell_command_line(
      "install the package manager before continuing"
    ));
  }
}
