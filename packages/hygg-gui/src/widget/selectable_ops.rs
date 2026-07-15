//! Pure helpers for the [`selectable`](super) widget: paragraph construction,
//! char/byte arithmetic, click bookkeeping and pointer hit-testing. Split out
//! of `selectable.rs` so each file stays within the repo's file-length gate.

use iced::advanced::text::{
  Hit, LineHeight, Paragraph, Shaping, Span, Text, Wrapping,
};
use iced::alignment::{Horizontal, Vertical};
use iced::mouse;
use iced::{Color, Font, Pixels, Point, Rectangle, Size};

use super::{Para, SelectionOwner, State};

/// Build the spans paragraph for `content`, highlighting the ordered char range
/// `sel` (empty when `sel.0 == sel.1`). Span index 1 is always the selection,
/// so `Paragraph::span_bounds(1)` yields the highlight rectangles.
pub fn build(
  content: &str,
  sel: (usize, usize),
  bounds: Size,
  size: f32,
  font: Font,
  color: Option<Color>,
) -> Para {
  let spans = spans_for(content, sel, color);
  Para::with_spans(Text {
    content: spans.as_slice(),
    bounds,
    size: Pixels(size),
    line_height: LineHeight::default(),
    font,
    horizontal_alignment: Horizontal::Left,
    vertical_alignment: Vertical::Top,
    shaping: Shaping::Advanced,
    wrapping: Wrapping::default(),
  })
}

/// One span for no selection, else three split at the selection's byte offsets
/// (leading / selected / trailing — empty ends are fine).
fn spans_for(
  content: &str,
  (s, e): (usize, usize),
  color: Option<Color>,
) -> Vec<Span<'_, (), Font>> {
  if s >= e {
    return vec![Span::new(content).color_maybe(color)];
  }
  let a = char_to_byte(content, s);
  let b = char_to_byte(content, e);
  vec![
    Span::new(&content[..a]).color_maybe(color),
    Span::new(&content[a..b]).color_maybe(color),
    Span::new(&content[b..]).color_maybe(color),
  ]
}

/// Byte offset of char index `off` in `content` (its byte length if past end).
pub fn char_to_byte(content: &str, off: usize) -> usize {
  content.char_indices().nth(off).map_or(content.len(), |(b, _)| b)
}

/// Char index of byte offset `byte` in `content` (its char count if past end).
/// `Paragraph::hit_test` returns a *byte* offset (cosmic-text's cursor index),
/// so this maps it back into the char space the widget selects in.
pub fn byte_to_char(content: &str, byte: usize) -> usize {
  content
    .char_indices()
    .position(|(b, _)| b >= byte)
    .unwrap_or_else(|| content.chars().count())
}

/// Order an anchor/cursor pair into `(start, end)` with `start <= end`.
pub fn ordered(a: usize, b: usize) -> (usize, usize) {
  (a.min(b), a.max(b))
}

/// The selected substring for the (unordered) char range `a..b`.
pub fn selected(content: &str, a: usize, b: usize) -> String {
  let (s, e) = ordered(a, b);
  content.chars().skip(s).take(e - s).collect()
}

/// Map the pointer to a char offset into `content`, or `None` when it is not
/// over the text. `hit_test` wants a point relative to the paragraph origin and
/// yields a byte offset, which [`byte_to_char`] maps into the char space.
pub fn locate(
  state: &State,
  bounds: Rectangle,
  cursor: mouse::Cursor,
  content: &str,
) -> Option<usize> {
  let p = cursor.position_over(bounds)?;
  let rel = Point::new(p.x - bounds.x, p.y - bounds.y);
  let byte = state.para.hit_test(rel).map(Hit::cursor)?;
  Some(byte_to_char(content, byte))
}

/// Bump the shared selection token and record it on `state`, making this widget
/// the sole owner of the highlight.
pub fn claim(owner: &SelectionOwner, state: &mut State) {
  let token = owner.get().wrapping_add(1);
  owner.set(token);
  state.owner = token;
}

/// Handle a left press at char offset `idx`. A single click drops the caret and
/// arms a drag; a double click (within 450 ms, ~same spot) selects the word; a
/// triple click selects everything. Returns whether the event was captured —
/// double/triple clicks are, so an underlying card button ignores them, while a
/// single click stays `Ignored` so the card still receives it.
pub fn press(
  state: &mut State,
  owner: &SelectionOwner,
  content: &str,
  idx: usize,
  chars: usize,
) -> bool {
  let now = crate::util::now_ms();
  let consecutive =
    now - state.last_ms < 450.0 && idx.abs_diff(state.click_at) <= 1;
  state.clicks = if consecutive { (state.clicks % 3) + 1 } else { 1 };
  state.last_ms = now;
  state.click_at = idx;
  let captured = match state.clicks {
    2 => {
      let (s, e) = crate::select::word_bounds(content, idx);
      state.anchor = s;
      state.cursor = e;
      true
    }
    3 => {
      state.anchor = 0;
      state.cursor = chars;
      true
    }
    _ => {
      state.anchor = idx;
      state.cursor = idx;
      state.pressed = true;
      state.drag = false;
      false
    }
  };
  claim(owner, state);
  captured
}
