use super::*;

#[test]
fn buffered_current_line_resets_after_background_setup() -> IoResult<()> {
  let mut editor = Editor::new(vec!["current".to_string()], 80);
  editor.cursor_y = 2;

  let mut buffer = Vec::new();
  assert!(editor.highlight_current_line(&mut buffer, 2, 4)?);

  let mut expected_tail = b"    ".to_vec();
  queue!(expected_tail, MoveTo(0, 2), ResetColor)?;

  assert!(
    buffer.ends_with(&expected_tail),
    "expected buffered highlight setup to fill the line, move back, then reset colors; got {:?}",
    String::from_utf8_lossy(&buffer)
  );

  Ok(())
}

use HighlightKind::{Persistent, Search, Selection, SpokenWord};

// Test ranges with literal byte columns; the writer test below feeds
// deliberately off-boundary ones.
fn r(start: usize, end: usize, kind: HighlightKind) -> HighlightRange {
  (ByteCol::lit(start), ByteCol::lit(end), kind)
}

#[test]
fn overlaps_resolve_to_the_highest_priority_kind() {
  // Persistent 0..10 under selection 4..8 under spoken word 6..7, plus a
  // search match 8..12 that only the persistent range sits beneath.
  let ranges = [
    r(0, 10, Persistent),
    r(4, 8, Selection),
    r(8, 12, Search),
    r(6, 7, SpokenWord),
  ];
  assert_eq!(
    resolve_overlaps(&ranges),
    vec![
      r(0, 4, Persistent),
      r(4, 6, Selection),
      r(6, 7, SpokenWord),
      r(7, 8, Selection),
      r(8, 12, Search),
    ]
  );
}

#[test]
fn adjacent_and_overlapping_ranges_of_one_kind_merge() {
  let ranges = [r(2, 5, Persistent), r(5, 7, Persistent), r(6, 9, Persistent)];
  assert_eq!(resolve_overlaps(&ranges), vec![r(2, 9, Persistent)]);
}

#[test]
fn writer_clamps_unsorted_and_out_of_range_input() {
  // 'ó' is bytes 5..7: the first range ends inside it, the second starts
  // before the first ended and runs past the line.
  let line = "canción";
  let ranges = [r(3, 6, Selection), r(1, 99, Search)];
  let mut out = Vec::new();
  write_highlighted_line(&mut out, line, &ranges, " ").unwrap();

  let mut want = b" can".to_vec();
  queue!(
    want,
    SetBackgroundColor(Color::DarkBlue),
    SetForegroundColor(Color::White)
  )
  .unwrap();
  want.extend_from_slice("ci".as_bytes());
  queue!(
    want,
    ResetColor,
    SetBackgroundColor(Color::Yellow),
    SetForegroundColor(Color::Black)
  )
  .unwrap();
  want.extend_from_slice("ón".as_bytes());
  queue!(want, ResetColor, Clear(ClearType::UntilNewLine)).unwrap();
  assert_eq!(String::from_utf8(out).unwrap(), String::from_utf8(want).unwrap());
}
