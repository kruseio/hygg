use crossterm::{
  QueueableCommand,
  cursor::{Hide, MoveTo, SetCursorStyle, Show},
  execute,
};
use std::io::{self, IsTerminal, Write};

use super::core::{Editor, EditorMode, ViewMode};

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

  // Calculate center position for cursor - called from main_loop
  pub fn center_cursor(&mut self) {
    // Always use normal mode centering
    self.debug_log("center_cursor: Using normal mode centering");
    self.center_cursor_with_overscroll(true)
  }

  // Calculate center position for cursor with optional overscroll
  pub fn center_cursor_with_overscroll(&mut self, allow_overscroll: bool) {
    // Get the actual line we're focusing on (absolute document position)
    let current_line = self.offset + self.cursor_y;

    // Ensure we don't go beyond document boundaries
    let current_line = current_line.min(self.total_lines.saturating_sub(1));

    // Calculate center position for cursor - place in middle of content area
    // (excluding status line)
    let content_height = self.height.saturating_sub(1);
    let center_y = content_height / 2;

    if allow_overscroll {
      // With overscroll, always try to center the current line on screen
      // This allows first and last lines to be centered with blank lines
      // above/below

      // Calculate the offset needed to center the current line
      let desired_offset = current_line.saturating_sub(center_y);

      // Allow negative offset conceptually by using the offset as signed
      if current_line < center_y {
        // We're in the overscroll region at the top
        self.offset = 0;
        self.cursor_y = current_line;
      } else {
        // Normal case or overscroll at bottom
        self.offset = desired_offset;
        self.cursor_y = center_y;
      }

      // No maximum offset limit - allow overscroll at the bottom too
      // This means we can have the last line centered with blank lines below
    } else {
      // Original logic without overscroll
      // First handle boundary cases
      if current_line < center_y {
        // Too close to the top to center properly
        self.offset = 0;
        self.cursor_y = current_line;
      } else if current_line >= self.total_lines.saturating_sub(center_y) {
        // Too close to the bottom to center properly
        if self.total_lines > content_height {
          self.offset = self.total_lines - content_height;
          self.cursor_y = current_line - self.offset;
        } else {
          // Document is shorter than screen height
          self.offset = 0;
          self.cursor_y = current_line;
        }
      } else {
        // Standard case - we can properly center
        self.offset = current_line - center_y;
        self.cursor_y = center_y;
      }

      // Final boundary check to ensure we don't go beyond document limits
      if self.offset + content_height > self.total_lines {
        self.offset = self.total_lines.saturating_sub(content_height);
      }
    }
    self.mark_dirty();
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::streaming::{PendingPdfStream, StreamReady};
  use std::sync::mpsc;
  use std::time::Instant;

  fn test_editor() -> Editor {
    let mut editor = Editor::new(vec!["line".to_string()], 80);
    editor.height = 24;
    editor.width = 80;
    editor.cursor_x = 0;
    editor.cursor_y = 0;
    editor
  }

  fn rendered(buffer: Vec<u8>) -> String {
    String::from_utf8(buffer).expect("cursor commands should be utf8")
  }

  #[test]
  fn buffered_cursor_hides_when_show_cursor_is_false() {
    let mut editor = test_editor();
    editor.cursor_currently_visible = true;
    editor.show_cursor = false;

    let mut buffer = Vec::new();
    editor.position_cursor_buffered(&mut buffer, 0).unwrap();

    assert!(rendered(buffer).contains("\x1b[?25l"));
    assert!(!editor.cursor_currently_visible);
  }

  #[test]
  fn buffered_cursor_hides_while_pdf_is_pending() {
    let mut editor = test_editor();
    editor.cursor_currently_visible = true;
    let (_tx, rx) = mpsc::channel::<StreamReady>();
    editor.pdf_pending = Some(PendingPdfStream {
      receiver: rx,
      started_at: Instant::now(),
      canonical_path_display: "pending.pdf".to_string(),
      restore_line_in_page: None,

    });

    let mut buffer = Vec::new();
    editor.position_cursor_buffered(&mut buffer, 0).unwrap();

    assert!(rendered(buffer).contains("\x1b[?25l"));
    assert!(!editor.cursor_currently_visible);
  }

  #[test]
  fn buffered_cursor_reuses_visible_state_and_cached_style() {
    let mut editor = test_editor();
    editor.cursor_currently_visible = true;
    editor.last_cursor_style = Some(SetCursorStyle::BlinkingBlock);
    editor.set_active_mode(EditorMode::Normal);

    let mut buffer = Vec::new();
    editor.position_cursor_buffered(&mut buffer, 0).unwrap();

    let output = rendered(buffer);
    assert!(!output.contains("\x1b[?25h"));
    assert!(!output.contains("\x1b[1 q"));
    assert!(editor.cursor_currently_visible);
    assert_eq!(editor.last_cursor_style, Some(SetCursorStyle::BlinkingBlock));
  }

  #[test]
  fn buffered_cursor_queues_style_when_mode_changes() {
    let mut editor = test_editor();
    editor.cursor_currently_visible = true;
    editor.last_cursor_style = Some(SetCursorStyle::BlinkingBlock);
    editor.set_active_mode(EditorMode::VisualChar);

    let mut buffer = Vec::new();
    editor.position_cursor_buffered(&mut buffer, 0).unwrap();

    let output = rendered(buffer);
    assert!(output.contains("\x1b[2 q"));
    assert!(!output.contains("\x1b[?25h"));
    assert_eq!(editor.last_cursor_style, Some(SetCursorStyle::SteadyBlock));
  }
}
