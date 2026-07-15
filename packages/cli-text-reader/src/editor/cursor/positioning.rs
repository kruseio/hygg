use crossterm::{
  QueueableCommand,
  cursor::{Hide, MoveTo, SetCursorStyle, Show},
  execute,
};
use std::io::{self, IsTerminal, Write};

use super::super::core::{Editor, EditorMode, ViewMode};

impl Editor {
  fn hide_cursor_buffered(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
    if self.cursor_currently_visible {
      buffer.queue(Hide)?;
      self.cursor_currently_visible = false;
    }
    Ok(())
  }

  fn queue_cursor_style_if_changed(
    &mut self,
    buffer: &mut Vec<u8>,
    style: SetCursorStyle,
  ) -> io::Result<()> {
    if self.last_cursor_style != Some(style) {
      buffer.queue(style)?;
      self.last_cursor_style = Some(style);
    }
    Ok(())
  }

  fn show_cursor_buffered(&mut self, buffer: &mut Vec<u8>) -> io::Result<()> {
    if !self.cursor_currently_visible {
      buffer.queue(Show)?;
      self.cursor_currently_visible = true;
    }
    Ok(())
  }

  // Position and style the cursor based on editor mode
  #[allow(dead_code)]
  pub fn position_cursor(
    &self,
    stdout: &mut io::Stdout,
    center_offset: usize,
  ) -> io::Result<()> {
    if std::io::stdout().is_terminal() {
      if !self.show_cursor {
        // Explicitly hide the cursor when show_cursor is false
        execute!(stdout, Hide)?;
        return Ok(());
      }
      // Keep the cursor hidden while the PDF is still being opened in the
      // background. The highlight bar already marks the row the cursor will
      // land on; the cursor itself only appears once real content is in.
      if self.pdf_pending.is_some() {
        execute!(stdout, Hide)?;
        return Ok(());
      }
      let active_mode = self.get_active_mode();
      // Position the cursor at current position in text
      if active_mode == EditorMode::Normal
        || active_mode == EditorMode::VisualChar
        || active_mode == EditorMode::VisualLine
      {
        // Calculate cursor position based on view mode
        let cursor_y = if self.view_mode == ViewMode::HorizontalSplit {
          // In split view, determine which pane the active buffer is in
          let is_in_top_pane = if self.tutorial_active && self.buffers.len() > 2
          {
            // Tutorial mode: buffer 1 is in top pane, buffer 2 is in bottom
            self.active_buffer == 1
          } else {
            // Normal mode: buffer 0 is in top pane, buffer 1 is in bottom
            self.active_buffer == 0
          };

          if is_in_top_pane {
            // Top pane - cursor is within the top pane's viewport
            self.cursor_y
          } else {
            // Bottom pane - cursor is below the top pane and separator
            let top_height = (self.height.saturating_sub(1) as f32
              * self.split_ratio) as usize;
            top_height + 1 + self.cursor_y // +1 for separator line
          }
        } else {
          // Normal view - cursor position unchanged
          self.cursor_y
        };

        // Position cursor exactly on the highlighted line
        execute!(
          stdout,
          MoveTo((center_offset + self.cursor_x) as u16, cursor_y as u16)
        )?;

        // Set appropriate cursor style for the mode and show cursor
        match active_mode {
          EditorMode::Normal => {
            execute!(stdout, Show, SetCursorStyle::BlinkingBlock)?;
          }
          EditorMode::VisualChar | EditorMode::VisualLine => {
            execute!(stdout, Show, SetCursorStyle::SteadyBlock)?;
          }
          _ => {
            execute!(stdout, Show)?;
          }
        }
      } else if active_mode == EditorMode::Command
        || active_mode == EditorMode::CommandExecution
        || active_mode == EditorMode::Search
        || active_mode == EditorMode::ReverseSearch
      {
        // In command/search mode, position cursor at the correct position
        let cmd_len = match active_mode {
          EditorMode::Command | EditorMode::CommandExecution => {
            1 + self.get_active_command_cursor_pos()
          } /* ":" + cursor pos */
          EditorMode::Search => 1 + self.get_active_command_cursor_pos(), /* "/" + cursor pos */
          EditorMode::ReverseSearch => 1 + self.get_active_command_cursor_pos(), /* "?" + cursor pos */
          _ => 0,
        };

        if self.show_cursor {
          execute!(
            stdout,
            Show,
            MoveTo(cmd_len as u16, (self.height - 1) as u16),
            SetCursorStyle::BlinkingBar
          )?;
        } else {
          execute!(stdout, Hide)?;
        }
      }
    }
    Ok(())
  }

  // Buffered version of position_cursor - positions and shows cursor in one go
  pub fn position_cursor_buffered(
    &mut self,
    buffer: &mut Vec<u8>,
    center_offset: usize,
  ) -> io::Result<()> {
    if !self.show_cursor {
      return self.hide_cursor_buffered(buffer);
    }
    // Keep the cursor hidden while the PDF is still being opened in the
    // background. The highlight bar already marks the row the cursor will
    // land on; the cursor itself only appears once real content is in.
    if self.pdf_pending.is_some() {
      return self.hide_cursor_buffered(buffer);
    }

    let active_mode = self.get_active_mode();

    // Position the cursor at current position in text
    if active_mode == EditorMode::Normal
      || active_mode == EditorMode::VisualChar
      || active_mode == EditorMode::VisualLine
    {
      // Calculate cursor position based on view mode
      let cursor_y = if self.view_mode == ViewMode::HorizontalSplit {
        // In split view, determine which pane the active buffer is in
        let is_in_top_pane = if self.tutorial_active && self.buffers.len() > 2 {
          // Tutorial mode: buffer 1 is in top pane, buffer 2 is in bottom
          self.active_buffer == 1
        } else {
          // Normal mode: buffer 0 is in top pane, buffer 1 is in bottom
          self.active_buffer == 0
        };

        if is_in_top_pane {
          // Top pane - cursor is within the top pane's viewport
          self.cursor_y
        } else {
          // Bottom pane - cursor is below the top pane and separator
          let top_height =
            (self.height.saturating_sub(1) as f32 * self.split_ratio) as usize;
          top_height + 1 + self.cursor_y // +1 for separator line
        }
      } else {
        // Normal view - cursor position unchanged
        self.cursor_y
      };

      // Position cursor exactly on the highlighted line
      buffer.queue(MoveTo(
        (center_offset + self.cursor_x) as u16,
        cursor_y as u16,
      ))?;

      // Set appropriate cursor style for the mode and show cursor
      if let Some(style) = match active_mode {
        EditorMode::Normal => Some(SetCursorStyle::BlinkingBlock),
        EditorMode::VisualChar | EditorMode::VisualLine => {
          Some(SetCursorStyle::SteadyBlock)
        }
        _ => None,
      } {
        self.queue_cursor_style_if_changed(buffer, style)?;
      }

      self.show_cursor_buffered(buffer)?;
    } else if active_mode == EditorMode::Command
      || active_mode == EditorMode::CommandExecution
      || active_mode == EditorMode::Search
      || active_mode == EditorMode::ReverseSearch
    {
      // In command/search mode, position cursor at the correct position
      let cmd_len = match active_mode {
        EditorMode::Command | EditorMode::CommandExecution => {
          1 + self.get_active_command_cursor_pos()
        } /* ":" + cursor pos */
        EditorMode::Search => 1 + self.get_active_command_cursor_pos(), /* "/" + cursor pos */
        EditorMode::ReverseSearch => 1 + self.get_active_command_cursor_pos(), /* "?" + cursor pos */
        _ => 0,
      };

      buffer.queue(MoveTo(cmd_len as u16, (self.height - 1) as u16))?;
      self
        .queue_cursor_style_if_changed(buffer, SetCursorStyle::BlinkingBar)?;
      self.show_cursor_buffered(buffer)?;
    }

    Ok(())
  }
}
