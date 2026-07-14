//! The `:` command line on the standalone home screen: quit, open a new file
//! by path, or remove the selected document. Kept separate from the picker's
//! event loop and rendering so each file stays small and focused.

use std::path::PathBuf;

use super::render::HomeItem;
use crate::library::remove_document;

/// Outcome of executing a `:` command line on the home screen.
pub(super) enum HomeCmd {
  Quit,
  Open(String),
  Status(String),
  None,
}

/// Run a `:` command typed on the home screen.
pub(super) fn execute_home_command(
  line: &str,
  items: &mut Vec<HomeItem>,
  selected: &mut usize,
) -> HomeCmd {
  let line = line.trim();
  let mut parts = line.splitn(2, char::is_whitespace);
  let command = parts.next().unwrap_or("");
  let arg = parts.next().unwrap_or("").trim();
  match command {
    "" => HomeCmd::None,
    "q" | "quit" | "exit" => HomeCmd::Quit,
    "open" | "o" | "e" | "edit" => {
      if arg.is_empty() {
        HomeCmd::Status("Usage: :open <file>".to_string())
      } else if let Some(path) = resolve_open_path(arg) {
        HomeCmd::Open(path)
      } else {
        HomeCmd::Status(format!("No such file: {arg}"))
      }
    }
    "remove" | "delete" | "rm" | "d" => {
      HomeCmd::Status(remove_selected(items, selected))
    }
    other => HomeCmd::Status(format!("Unknown command: :{other}")),
  }
}

/// Remove the selected document (local-only) and return a status line. Removes
/// only the library listing (and hygg's own cache copy) — never the user's
/// source file or the server copy.
pub(super) fn remove_selected(
  items: &mut Vec<HomeItem>,
  selected: &mut usize,
) -> String {
  let Some(item) = items.get(*selected) else {
    return String::new();
  };
  let title = item.entry.title.clone();
  match remove_document(&item.entry) {
    Ok(()) => {
      items.remove(*selected);
      *selected = (*selected).min(items.len().saturating_sub(1));
      format!("Removed “{title}”.")
    }
    Err(e) => format!("Couldn’t remove “{title}”: {e}"),
  }
}

/// Resolve a `:open` argument to an existing file path, expanding a leading
/// `~`. Returns `None` when the file does not exist.
fn resolve_open_path(arg: &str) -> Option<String> {
  let path: PathBuf = if arg == "~" {
    dirs::home_dir()?
  } else if let Some(rest) = arg.strip_prefix("~/") {
    dirs::home_dir()?.join(rest)
  } else {
    PathBuf::from(arg)
  };
  path.is_file().then(|| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn quit_command_is_recognized() {
    for word in ["q", "quit", "exit", "  q  "] {
      assert!(matches!(
        execute_home_command(word, &mut Vec::new(), &mut 0),
        HomeCmd::Quit
      ));
    }
  }

  #[test]
  fn open_command_returns_existing_path() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    let cmd = format!("open {path}");
    match execute_home_command(&cmd, &mut Vec::new(), &mut 0) {
      HomeCmd::Open(p) => assert_eq!(p, path),
      other => panic!("expected Open, got {:?}", DebugCmd(&other)),
    }
  }

  #[test]
  fn open_missing_file_reports_status() {
    let cmd = "open /no/such/file/here.txt";
    assert!(matches!(
      execute_home_command(cmd, &mut Vec::new(), &mut 0),
      HomeCmd::Status(_)
    ));
  }

  #[test]
  fn open_without_argument_shows_usage() {
    match execute_home_command("open", &mut Vec::new(), &mut 0) {
      HomeCmd::Status(s) => assert!(s.contains("Usage")),
      other => panic!("expected Status, got {:?}", DebugCmd(&other)),
    }
  }

  #[test]
  fn unknown_command_reports_status() {
    match execute_home_command("frobnicate", &mut Vec::new(), &mut 0) {
      HomeCmd::Status(s) => assert!(s.contains("Unknown")),
      other => panic!("expected Status, got {:?}", DebugCmd(&other)),
    }
  }

  #[test]
  fn resolve_open_path_requires_existence() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    assert_eq!(resolve_open_path(&path), Some(path));
    assert_eq!(resolve_open_path("/definitely/not/here"), None);
  }

  /// Test-only wrapper so panics can print the non-`Debug` `HomeCmd`.
  struct DebugCmd<'a>(&'a HomeCmd);
  impl std::fmt::Debug for DebugCmd<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      let name = match self.0 {
        HomeCmd::Quit => "Quit",
        HomeCmd::Open(_) => "Open",
        HomeCmd::Status(_) => "Status",
        HomeCmd::None => "None",
      };
      f.write_str(name)
    }
  }
}
