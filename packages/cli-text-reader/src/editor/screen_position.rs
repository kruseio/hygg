use super::core::Editor;

impl Editor {
  pub fn get_screen_position(&self, position: char) -> (usize, usize) {
    let viewport_height = self.height.saturating_sub(1);

    match position {
      'H' => {
        // High - top of screen
        (self.offset, 0)
      }
      'M' => {
        // Middle - middle of screen
        let middle_line = self.offset + (viewport_height / 2);
        let target_line = middle_line.min(self.lines.len().saturating_sub(1));
        (target_line, 0)
      }
      'L' => {
        // Low - bottom of screen
        let bottom_line = self.offset + viewport_height.saturating_sub(1);
        let target_line = bottom_line.min(self.lines.len().saturating_sub(1));
        (target_line, 0)
      }
      _ => self.get_cursor_position(),
    }
  }
}
