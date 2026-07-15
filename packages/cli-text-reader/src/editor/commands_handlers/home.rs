use super::super::core::{Editor, ViewMode};

impl Editor {
  /// `:home` / `:Rex` — leave the current document and return to the
  /// full-screen home library picker (the same view shown when hygg is
  /// launched with no input file). This ends the reader loop cleanly, saving
  /// the reading position first; the launcher then re-enters the picker so the
  /// user can resume any document or open a new one. (`:Rex` echoes Vim's
  /// netrw "Return to Explorer".)
  pub fn handle_home_command(
    &mut self,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    // Unwind any transient view (help/notes/credits overlay, shell split,
    // tutorial) back to the underlying document so the position saved on the
    // way out belongs to the document, not the overlay's scratch buffer.
    loop {
      match self.view_mode {
        ViewMode::HorizontalSplit => {
          self.close_split();
        }
        ViewMode::Overlay => {
          self.close_overlay();
        }
        ViewMode::Normal => break,
      }
    }
    self.exit_to_home = true;
    Ok(true)
  }
}
