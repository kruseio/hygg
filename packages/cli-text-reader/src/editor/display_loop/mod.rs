mod input;
mod main_loop;
mod render;
mod tick;

#[cfg(test)]
mod tests;

pub(crate) const FAST_EVENT_POLL_MS: u64 = 16;
pub(crate) const PDF_LOADING_EVENT_POLL_MS: u64 = 120;
pub(crate) const IDLE_EVENT_POLL_MS: u64 = 250;

/// Signals from an input-handling step back to the main loop.
pub(crate) enum LoopControl {
  /// Continue to the next main-loop iteration immediately.
  Continue,
  /// Break out of the main loop (exit the editor).
  Break,
  /// Fall through to the rest of the current iteration.
  Proceed,
}

pub(crate) fn event_poll_timeout(
  needs_redraw: bool,
  tutorial_demo_mode: bool,
  streaming_active: bool,
  pending_pdf: bool,
  load_transitioning: bool,
) -> std::time::Duration {
  if needs_redraw || tutorial_demo_mode {
    std::time::Duration::from_millis(FAST_EVENT_POLL_MS)
  } else if streaming_active || pending_pdf || load_transitioning {
    std::time::Duration::from_millis(PDF_LOADING_EVENT_POLL_MS)
  } else {
    std::time::Duration::from_millis(IDLE_EVENT_POLL_MS)
  }
}
