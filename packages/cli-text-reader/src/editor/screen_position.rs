use super::char_boundary::{ByteCol, Pos};
use super::core::Editor;

impl Editor {
  pub fn get_screen_position(&self, position: char) -> Pos {
    let viewport_height = self.height.saturating_sub(1);

    match position {
      'H' => {
        // High - top of screen
        Pos::new(self.offset, ByteCol::ZERO)
      }
      'M' => {
        // Middle - middle of screen
        let middle_line = self.offset + (viewport_height / 2);
        let target_line = middle_line.min(self.lines.len().saturating_sub(1));
        Pos::new(target_line, ByteCol::ZERO)
      }
      'L' => {
        // Low - bottom of screen
        let bottom_line = self.offset + viewport_height.saturating_sub(1);
        let target_line = bottom_line.min(self.lines.len().saturating_sub(1));
        Pos::new(target_line, ByteCol::ZERO)
      }
      _ => self.get_cursor_position(),
    }
  }
}
