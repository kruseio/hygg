use std::io::{self, Write};

use crossterm::{
  QueueableCommand,
  cursor::MoveTo,
  terminal::{Clear, ClearType},
};

/// Contract every element of the right-anchored status bar must satisfy.
///
/// Declaring `reserved_width` is mandatory: the bar packs elements by these
/// widths, so a new element cannot be added without sizing it, and an
/// element's column never moves when only its *contents* change width. The
/// reserved width may depend on editor state (e.g. the page count) but must be
/// the widest the element can render in that state.
pub(crate) trait StatusSlot {
  /// Columns to reserve for this element, whether it renders this frame or
  /// not — that is what keeps neighbouring elements from shifting.
  fn reserved_width(&self) -> usize;

  /// Text to draw this frame, or `None` to leave the reserved columns
  /// untouched so document content shows through. It is right-aligned within
  /// `reserved_width` and must never be wider than it.
  fn render(&self) -> Option<String>;
}

/// Start column for each slot when packed right-to-left: the first slot's
/// right edge sits `right_margin` columns from `terminal_width`, and each
/// later slot is placed `gap` columns further left. Saturates at 0 so a narrow
/// terminal clamps instead of underflowing.
pub(crate) fn pack_right_anchored(
  terminal_width: usize,
  right_margin: usize,
  gap: usize,
  reserved: &[usize],
) -> Vec<usize> {
  let mut x = terminal_width.saturating_sub(right_margin);
  let mut starts = Vec::with_capacity(reserved.len());
  for (idx, &width) in reserved.iter().enumerate() {
    if idx > 0 {
      x = x.saturating_sub(gap);
    }
    x = x.saturating_sub(width);
    starts.push(x);
  }
  starts
}

/// Draw a right-anchored row of fixed-width slots (given rightmost-first) at
/// row `y`. Each rendered slot is right-aligned within its reserved width; the
/// rightmost slot also clears to the end of the line when it renders, wiping
/// the trailing margin.
pub(crate) fn draw_right_anchored<W: Write>(
  out: &mut W,
  terminal_width: usize,
  y: u16,
  right_margin: usize,
  gap: usize,
  slots: &[&dyn StatusSlot],
) -> io::Result<()> {
  let reserved: Vec<usize> =
    slots.iter().map(|slot| slot.reserved_width()).collect();
  let starts =
    pack_right_anchored(terminal_width, right_margin, gap, &reserved);

  // Draw left-to-right so the rightmost slot (index 0) — the only one that
  // clears to the end of the line — is written last.
  for idx in (0..slots.len()).rev() {
    let Some(text) = slots[idx].render() else {
      continue;
    };
    let width = reserved[idx];
    out.queue(MoveTo(starts[idx] as u16, y))?;
    write!(out, "{text:>width$}")?;
    if idx == 0 {
      out.queue(Clear(ClearType::UntilNewLine))?;
    }
  }
  Ok(())
}
