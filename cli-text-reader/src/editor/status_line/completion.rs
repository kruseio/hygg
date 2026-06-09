use crossterm::{
  QueueableCommand, execute,
  style::{Color, ResetColor, SetForegroundColor},
};
use std::io::{self, Write};

use super::super::core::Editor;

impl Editor {
  pub(crate) fn draw_command_completion(
    &self,
    stdout: &mut io::Stdout,
    occupied_width: usize,
  ) -> io::Result<()> {
    let Some(completion) = self.command_completion_text(occupied_width) else {
      return Ok(());
    };
    execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
    write!(stdout, "{completion}")?;
    execute!(stdout, ResetColor)?;
    Ok(())
  }

  pub(crate) fn draw_command_completion_buffered(
    &self,
    buffer: &mut Vec<u8>,
    occupied_width: usize,
  ) -> io::Result<()> {
    let Some(completion) = self.command_completion_text(occupied_width) else {
      return Ok(());
    };
    buffer.queue(SetForegroundColor(Color::DarkGrey))?;
    write!(buffer, "{completion}")?;
    buffer.queue(ResetColor)?;
    Ok(())
  }

  pub(crate) fn command_completion_text(
    &self,
    occupied_width: usize,
  ) -> Option<String> {
    let completion = self.editor_state.command_completion.as_deref()?.trim();
    if completion.is_empty() {
      return None;
    }

    let separator_width = 2;
    let available =
      self.width.saturating_sub(occupied_width).saturating_sub(separator_width);
    if available == 0 {
      return None;
    }

    let mut completion = completion.to_string();
    completion.truncate(available);
    Some(format!("  {completion}"))
  }
}
