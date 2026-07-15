use crossterm::{
  cursor::{Hide, MoveTo},
  execute,
  terminal::{self, Clear, ClearType},
};
use std::io::{self, IsTerminal, Write};

use super::core::Editor;
use crate::tutorial::get_tutorial_text;

impl Editor {
  #[allow(dead_code)]
  pub fn show_tutorial(
    &self,
    stdout: &mut io::Stdout,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let tutorial_lines = get_tutorial_text();

    if std::io::stdout().is_terminal() {
      // Save current state
      let was_raw = terminal::is_raw_mode_enabled()?;

      if !was_raw {
        terminal::enable_raw_mode()?;
      }
      execute!(stdout, Hide)?;

      let mut tutorial_offset = 0;
      loop {
        // Display tutorial with scrolling
        execute!(stdout, Clear(ClearType::All))?;
        let center_offset = if self.width > self.col {
          (self.width / 2) - self.col / 2
        } else {
          0
        };

        for (i, line) in tutorial_lines
          .iter()
          .skip(tutorial_offset)
          .take(self.height)
          .enumerate()
        {
          execute!(stdout, MoveTo(center_offset as u16, i as u16))?;
          println!("{line}");
        }

        stdout.flush()?;

        // Handle scrolling input
        if let crossterm::event::Event::Key(key_event) =
          crossterm::event::read()?
        {
          match key_event.code {
            crossterm::event::KeyCode::Char('j')
            | crossterm::event::KeyCode::Down => {
              if tutorial_offset + self.height < tutorial_lines.len() {
                tutorial_offset += 1;
              }
            }
            crossterm::event::KeyCode::Char('k')
            | crossterm::event::KeyCode::Up => {
              tutorial_offset = tutorial_offset.saturating_sub(1);
            }
            crossterm::event::KeyCode::PageDown => {
              tutorial_offset = (tutorial_offset + self.height)
                .min(tutorial_lines.len().saturating_sub(self.height));
            }
            crossterm::event::KeyCode::PageUp => {
              tutorial_offset = tutorial_offset.saturating_sub(self.height);
            }
            _ => break,
          }
        }
      }

      // Restore original state
      execute!(stdout, Clear(ClearType::All))?;
      if !was_raw {
        terminal::disable_raw_mode()?;
      }
    }

    Ok(())
  }
}
