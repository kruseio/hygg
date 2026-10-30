use super::*;

// "canción €": 'ó' is bytes 5..7, '€' 9..12, the line is 12 bytes.
const LINE: &str = "canción €";
// a 0, e 1, U+0301 2..4, 中 4..7, 👩 7..11, ZWJ 11..14, 💻 14..18, z 18
const CLUSTERS: &str = "ae\u{301}中👩\u{200d}💻z";

#[test]
fn constructors_settle_or_reject_off_boundary_offsets() {
  assert_eq!(ByteCol::floor(LINE, 6), ByteCol::floor(LINE, 5));
  assert_eq!(ByteCol::floor(LINE, 6).raw(), 5);
  assert_eq!(ByteCol::floor(LINE, 99), ByteCol::end(LINE));
  assert_eq!(ByteCol::checked(LINE, 5).map(ByteCol::raw), Some(5));
  assert_eq!(ByteCol::checked(LINE, 6), None);
  assert_eq!(ByteCol::checked(LINE, 13), None);
  assert_eq!(ByteCol::end(LINE).raw(), 12);
  assert_eq!(ByteCol::ZERO.raw(), 0);
  assert_eq!(ByteCol::last_grapheme(CLUSTERS).raw(), 18);
  // A stale column from a longer line is re-floored onto this one.
  assert_eq!(ByteCol::floor("canción €€", 13).settle(LINE).raw(), 12);
}

#[test]
fn conversions_between_units_are_explicit_and_round_trip() {
  let col = ByteCol::floor(LINE, 9);
  assert_eq!(col.char_index(LINE), 8);
  assert_eq!(ByteCol::from_char_index(LINE, 8), col);
  assert_eq!(col.grapheme_index(LINE), 8);
  assert_eq!(ByteCol::from_grapheme_index(LINE, 8), col);
  assert_eq!(col.char_at(LINE), Some('€'));
  // Graphemes and cells: 中 starts at cell 2, the ZWJ emoji at cell 4.
  let emoji = ByteCol::floor(CLUSTERS, 7);
  assert_eq!(emoji.grapheme_index(CLUSTERS), 3);
  assert_eq!(emoji.display_col(CLUSTERS), DisplayCol::new(4));
  assert_eq!(ByteCol::from_display_col(CLUSTERS, DisplayCol::new(5)), emoji);
  assert_eq!(
    ByteCol::from_display_col(CLUSTERS, DisplayCol::END),
    ByteCol::last_grapheme(CLUSTERS)
  );
}

#[test]
fn steps_move_by_whole_characters_or_clusters() {
  let col = ByteCol::floor(CLUSTERS, 1);
  assert_eq!(col.next_char(CLUSTERS).raw(), 2);
  assert_eq!(col.next_grapheme(CLUSTERS).raw(), 4);
  assert_eq!(ByteCol::floor(CLUSTERS, 14).floor_grapheme(CLUSTERS).raw(), 7);
  assert_eq!(ByteCol::floor(CLUSTERS, 18).prev_grapheme(CLUSTERS).raw(), 7);
}

#[test]
fn cursor_col_only_sits_on_grapheme_starts() {
  let mid = ByteCol::floor(CLUSTERS, 11); // the ZWJ
  assert_eq!(CursorCol::floor(CLUSTERS, mid).raw(), 7);
  assert_eq!(
    CursorCol::clamped(CLUSTERS, ByteCol::end(CLUSTERS)),
    CursorCol::last(CLUSTERS)
  );
  // `floor` keeps the end of the line; `clamped` does not.
  assert_eq!(CursorCol::floor(CLUSTERS, ByteCol::end(CLUSTERS)).raw(), 19);
  let c = CursorCol::floor(CLUSTERS, ByteCol::from_grapheme_index(CLUSTERS, 1));
  assert_eq!(c.raw(), 1);
  assert_eq!(c.next(CLUSTERS).raw(), 4);
  assert_eq!(c.next(CLUSTERS).prev(CLUSTERS), c);
  assert_eq!(ByteCol::from(c), c.col());
  assert_eq!(c.display_col(CLUSTERS), DisplayCol::new(1));
  assert_eq!(
    CursorCol::from_display_col(CLUSTERS, DisplayCol::new(3)).raw(),
    4
  );
  assert_eq!(CursorCol::ZERO.raw(), 0);
}

#[test]
fn find_and_iterate_with_typed_columns() {
  let cols: Vec<usize> = char_starts(LINE).map(|(c, _)| c.raw()).collect();
  assert_eq!(cols, vec![0, 1, 2, 3, 4, 5, 7, 8, 9]);
  assert_eq!(
    find_char_from(LINE, ByteCol::floor(LINE, 6), '€').map(ByteCol::raw),
    Some(9)
  );
  assert_eq!(find_char_from(LINE, ByteCol::end(LINE), '€'), None);
  assert_eq!(
    rfind_char_before(LINE, ByteCol::end(LINE), 'c').map(ByteCol::raw),
    Some(3)
  );
  assert_eq!(
    rfind_char_before(LINE, ByteCol::floor(LINE, 3), 'c').map(ByteCol::raw),
    Some(0)
  );
}

#[test]
fn positions_order_by_line_then_column_and_come_from_raw_safely() {
  let lines = vec![LINE.to_string(), "ab".to_string()];
  let a = Pos::new(0, ByteCol::floor(LINE, 9));
  let b = Pos::new(1, ByteCol::ZERO);
  assert!(a < b);
  // Mid-character and past-the-end raw columns are floored; a missing line
  // gets column 0.
  assert_eq!(
    Pos::from_raw(&lines, (0, 6)),
    Pos::new(0, ByteCol::floor(LINE, 5))
  );
  assert_eq!(Pos::from_raw(&lines, (1, 99)).to_raw(), (1, 2));
  assert_eq!(Pos::from_raw(&lines, (7, 3)).to_raw(), (7, 0));
  let span = LineSpan::new(1, ByteCol::ZERO, ByteCol::end("ab"));
  assert_eq!(Pos::new(span.line, span.start), b);
  assert_eq!(format!("{}", ByteCol::end(LINE)), "12");
  assert_eq!(format!("{}", CursorCol::last("ab")), "1");
}
