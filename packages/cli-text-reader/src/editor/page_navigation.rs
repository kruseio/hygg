use super::core::{Editor, ViewMode};
use crossterm::event::{KeyCode, KeyModifiers};

impl Editor {
  pub fn handle_page_navigation(
    &mut self,
    key_code: KeyCode,
    modifiers: KeyModifiers,
  ) -> Option<bool> {
    match key_code {
      KeyCode::PageDown => {
        // Move cursor down by page height to get overscroll behavior
        let current_line = self.offset + self.cursor_y;
        let viewport_height = self.get_effective_viewport_height();
        let page_size = viewport_height.saturating_sub(3); // Leave some overlap
        let target_line =
          (current_line + page_size).min(self.total_lines.saturating_sub(1));

        if target_line != current_line {
          self.goto_line_with_overscroll(target_line);
        }
        Some(false)
      }
      KeyCode::PageUp => {
        // Move cursor up by page height to get overscroll behavior
        let current_line = self.offset + self.cursor_y;
        let viewport_height = self.get_effective_viewport_height();
        let page_size = viewport_height.saturating_sub(3); // Leave some overlap
        let target_line = current_line.saturating_sub(page_size);

        if target_line != current_line {
          self.goto_line_with_overscroll(target_line);
        }
        Some(false)
      }
      KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+U - half page up
        self.half_page_up();
        Some(false)
      }
      KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
        // Ctrl+D - half page down
        self.half_page_down();
        Some(false)
      }
      _ => None,
    }
  }
}
