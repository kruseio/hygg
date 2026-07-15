use std::io::{self, IsTerminal};

use super::super::core::{Editor, SnapshotReason};
use super::LoopControl;

impl Editor {
  pub fn main_loop(
    &mut self,
    stdout: &mut io::Stdout,
    skip_first_center: bool,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let mut first_iteration = true;

    loop {
      // Per-iteration housekeeping: background PDF polling, TTS narration
      // advancement, the load indicator, and debug logging.
      self.pre_render_tick();

      // Render the frame (or just keep the idle cursor positioned).
      self.render_frame(stdout, first_iteration, skip_first_center)?;

      first_iteration = false;
      self.initial_setup_complete = true;

      // Handle keyboard input
      let control = if std::io::stdout().is_terminal() {
        self.handle_input_terminal(stdout)?
      } else {
        self.handle_input_non_terminal(stdout)?
      };
      match control {
        LoopControl::Continue => continue,
        LoopControl::Break => break,
        LoopControl::Proceed => {}
      }

      self.save_progress_snapshot(SnapshotReason::Passive)?;
      self.debug_log("Main loop iteration complete\n");
    }

    self.save_progress_snapshot(SnapshotReason::Exit)?;
    Ok(())
  }
}
