use crossterm::{
  cursor::{Hide, MoveTo, Show},
  event::{self, Event, KeyCode, KeyEventKind},
  execute, queue,
  style::{Attribute, Print, SetAttribute},
  terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
  },
};
use std::io::{self, IsTerminal, Write};

use super::command::{HomeCmd, execute_home_command, remove_selected};
use super::render::{HomeItem, item_card, stats_line};
use super::sync::reconcile_home_items;

/// Header rows before the card list: "hygg", a blank, the stats line, a blank.
const HEADER_ROWS: usize = 4;
/// Terminal rows one card occupies: title, meta, and a trailing blank.
const CARD_ROWS: usize = 3;

/// The `:home` landing screen shown when hygg is launched with no input file.
/// Reconciles reading progress with the server (last-write-wins, persisted both
/// ways), then lets the user resume a document, remove one, or open a new file
/// with `:open <file>`. Returns the path to open, or `None` if the user quits.
pub fn run_home(_col: usize) -> io::Result<Option<String>> {
  if !io::stdout().is_terminal() {
    return Ok(None);
  }
  // Pull + reconcile before entering the alternate screen so the list is
  // already up to date (and matches every other device) on first paint.
  let items = reconcile_home_items();
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen, Hide)?;
  terminal::enable_raw_mode()?;
  let result = event_loop(&mut stdout, items);
  terminal::disable_raw_mode()?;
  execute!(stdout, Show, LeaveAlternateScreen)?;
  result
}

fn event_loop(
  stdout: &mut io::Stdout,
  mut items: Vec<HomeItem>,
) -> io::Result<Option<String>> {
  let mut selected = 0usize;
  let mut top = 0usize;
  // `Some(buffer)` while the user is typing a `:` command; `None` in normal
  // (navigation) mode.
  let mut command: Option<String> = None;
  let mut status = String::new();
  loop {
    let (width, height) = terminal::size().unwrap_or((80, 24));
    let visible = visible_cards(height as usize);
    top = scroll_top(top, selected, visible);
    render(
      stdout,
      &items,
      selected,
      top,
      width as usize,
      visible,
      command.as_deref(),
      &status,
    )?;

    let Event::Key(key) = event::read()? else {
      continue;
    };
    if key.kind == KeyEventKind::Release {
      continue;
    }

    if let Some(buffer) = command.as_mut() {
      match key.code {
        KeyCode::Esc => command = None,
        // Backspace deletes a char; backspacing past the empty prompt cancels.
        KeyCode::Backspace if buffer.pop().is_none() => command = None,
        KeyCode::Char(c) => buffer.push(c),
        KeyCode::Enter => {
          let line = std::mem::take(buffer);
          command = None;
          match execute_home_command(&line, &mut items, &mut selected) {
            HomeCmd::Quit => return Ok(None),
            HomeCmd::Open(path) => return Ok(Some(path)),
            HomeCmd::Status(message) => status = message,
            HomeCmd::None => {}
          }
        }
        _ => {}
      }
      continue;
    }

    status.clear();
    match key.code {
      KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
      KeyCode::Char('j') | KeyCode::Down if selected + 1 < items.len() => {
        selected += 1;
      }
      KeyCode::Char('k') | KeyCode::Up => {
        selected = selected.saturating_sub(1);
      }
      KeyCode::Char('g') => selected = 0,
      KeyCode::Char('G') => selected = items.len().saturating_sub(1),
      KeyCode::Char('x') | KeyCode::Delete => {
        status = remove_selected(&mut items, &mut selected);
      }
      KeyCode::Char(':') => command = Some(String::new()),
      KeyCode::Enter => {
        if let Some(path) =
          items.get(selected).and_then(|i| i.entry.source_path.clone())
        {
          return Ok(Some(path));
        }
      }
      _ => {}
    }
  }
}

/// How many cards fit on screen, leaving room for the header and footer.
fn visible_cards(height: usize) -> usize {
  (height.saturating_sub(HEADER_ROWS + 1) / CARD_ROWS).max(1)
}

/// Keep the selected card within the visible window.
fn scroll_top(top: usize, selected: usize, visible: usize) -> usize {
  if selected < top {
    selected
  } else if selected >= top + visible {
    selected + 1 - visible
  } else {
    top
  }
}

#[allow(clippy::too_many_arguments)]
fn render(
  stdout: &mut io::Stdout,
  items: &[HomeItem],
  selected: usize,
  top: usize,
  width: usize,
  visible: usize,
  command: Option<&str>,
  status: &str,
) -> io::Result<()> {
  queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
  queue!(stdout, Print("  hygg\r\n\r\n"))?;
  queue!(stdout, Print(format!("{}\r\n\r\n", stats_line(items))))?;
  if items.is_empty() {
    queue!(stdout, Print("  Your library is empty.\r\n"))?;
    queue!(stdout, Print("  Open a document with:  :open <file>\r\n"))?;
  } else {
    let body_width = width.saturating_sub(4).max(8);
    for (idx, item) in items.iter().enumerate().skip(top).take(visible) {
      let [title, meta] = item_card(item, body_width);
      queue_card_line(stdout, &title, idx == selected)?;
      queue_card_line(stdout, &meta, idx == selected)?;
      queue!(stdout, Print("\r\n"))?;
    }
  }
  let footer = match (command, status) {
    (Some(buffer), _) => format!("  :{buffer}"),
    (None, s) if !s.is_empty() => format!("  {s}"),
    _ => {
      "  j/k move · Enter open · x remove · :open <file> · q quit".to_string()
    }
  };
  let bottom = terminal::size().map(|(_, h)| h).unwrap_or(24).saturating_sub(1);
  queue!(
    stdout,
    MoveTo(0, bottom),
    Clear(ClearType::CurrentLine),
    Print(footer)
  )?;
  stdout.flush()
}

/// Draw one card line, reverse-highlighted when it belongs to the selection.
fn queue_card_line(
  stdout: &mut io::Stdout,
  text: &str,
  selected: bool,
) -> io::Result<()> {
  if selected {
    queue!(
      stdout,
      SetAttribute(Attribute::Reverse),
      Print(format!("  {text}")),
      SetAttribute(Attribute::Reset),
      Print("\r\n")
    )
  } else {
    queue!(stdout, Print(format!("  {text}\r\n")))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn windowing_keeps_selection_visible() {
    assert!(visible_cards(24) >= 1);
    assert_eq!(visible_cards(0), 1);
    // selection below the window scrolls down; above scrolls up.
    assert_eq!(scroll_top(0, 5, 3), 3);
    assert_eq!(scroll_top(4, 2, 3), 2);
    assert_eq!(scroll_top(2, 3, 3), 2);
  }
}
