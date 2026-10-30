use super::*;

// "canción": c a n c i ó n — 'ó' is bytes 5..7, the line is 8 bytes.
const LINE: &str = "canción";

#[test]
fn floor_and_ceil_settle_inside_a_multibyte_char() {
  assert_eq!(floor_char_boundary(LINE, 6), 5);
  assert_eq!(ceil_char_boundary(LINE, 6), 7);
  assert_eq!(floor_char_boundary(LINE, 5), 5);
  assert_eq!(ceil_char_boundary(LINE, 5), 5);
  assert_eq!(floor_char_boundary(LINE, 99), LINE.len());
  assert_eq!(ceil_char_boundary(LINE, 99), LINE.len());
}

#[test]
fn next_and_prev_step_whole_characters() {
  assert_eq!(next_char_boundary(LINE, 4), 5);
  assert_eq!(next_char_boundary(LINE, 5), 7);
  assert_eq!(next_char_boundary(LINE, 6), 7);
  assert_eq!(next_char_boundary(LINE, 7), 8);
  assert_eq!(next_char_boundary(LINE, 8), 8);
  assert_eq!(prev_char_boundary(LINE, 7), 5);
  assert_eq!(prev_char_boundary(LINE, 5), 4);
  assert_eq!(prev_char_boundary(LINE, 0), 0);
}

#[test]
fn columns_count_characters_not_bytes() {
  assert_eq!(char_at(LINE, 5), Some('ó'));
  assert_eq!(char_at(LINE, 6), Some('ó'));
  assert_eq!(char_at(LINE, 8), None);
  assert_eq!(char_column(LINE, 7), 6);
  assert_eq!(char_column(LINE, 6), 5);
  assert_eq!(byte_offset_of_char(LINE, 6), 7);
  assert_eq!(byte_offset_of_char(LINE, 99), LINE.len());
}

#[test]
fn safe_slice_bounds_always_slices() {
  for start in 0..=LINE.len() + 2 {
    for end in 0..=LINE.len() + 2 {
      let (s, e) = settle_bytes(LINE, start, end);
      let _ = &LINE[s..e];
      let _ = &LINE[..s];
      let _ = &LINE[e..];
    }
  }
}

#[test]
fn slice_helpers_match_indexing_on_valid_ranges() {
  for (start, _) in LINE.char_indices().chain([(LINE.len(), ' ')]) {
    for (end, _) in LINE.char_indices().chain([(LINE.len(), ' ')]) {
      if start <= end {
        assert_eq!(slice_bytes(LINE, start, end), &LINE[start..end]);
      }
    }
    assert_eq!(slice_bytes_from(LINE, start), &LINE[start..]);
    assert_eq!(slice_bytes_to(LINE, start), &LINE[..start]);
  }
}

#[test]
fn slice_helpers_clamp_invalid_ranges() {
  // 'ó' is bytes 5..7: an index inside it floors to 5.
  assert_eq!(slice_bytes(LINE, 3, 6), "ci");
  assert_eq!(slice_bytes(LINE, 6, 99), "ón");
  assert_eq!(slice_bytes(LINE, 7, 2), "");
  assert_eq!(slice_bytes_from(LINE, 6), "ón");
  assert_eq!(slice_bytes_from(LINE, 99), "");
  assert_eq!(slice_bytes_to(LINE, 6), "canci");
  assert_eq!(slice_bytes_to(LINE, 99), LINE);
}

// One visible character each, several chars each.
const COMBINING: &str = "e\u{301}"; // 3 bytes, width 1
const THAI: &str = "ที่"; // 9 bytes, width 1
// 6 bytes, width 2: the vowel sign is a spacing mark with a cell of its own
const DEVANAGARI: &str = "कि";
const ZWJ: &str = "👩\u{200d}💻"; // 11 bytes, width 2
const FLAG: &str = "🇪🇸"; // 8 bytes, width 2

#[test]
fn multi_char_graphemes_are_one_step() {
  for g in [COMBINING, THAI, DEVANAGARI, ZWJ, FLAG] {
    let line = format!("a{g}b");
    let b = 1 + g.len();
    assert_eq!(next_grapheme_boundary(&line, 0), 1, "{g:?}");
    assert_eq!(next_grapheme_boundary(&line, 1), b, "{g:?}");
    assert_eq!(prev_grapheme_boundary(&line, b), 1, "{g:?}");
    assert_eq!(prev_grapheme_boundary(&line, 1), 0, "{g:?}");
    // Every byte inside the grapheme floors to its start.
    for i in 1..b {
      assert_eq!(floor_grapheme_boundary(&line, i), 1, "{g:?} at {i}");
    }
    assert_eq!(last_grapheme_start(&format!("a{g}")), 1, "{g:?}");
  }
  assert_eq!(next_grapheme_boundary("ab", 2), 2);
  assert_eq!(prev_grapheme_boundary("ab", 0), 0);
  assert_eq!(last_grapheme_start(""), 0);
  assert_eq!(floor_grapheme_boundary("ab", 99), 2);
}

#[test]
fn two_flags_are_two_graphemes() {
  let line = format!("{FLAG}{FLAG}");
  assert_eq!(next_grapheme_boundary(&line, 0), 8);
  assert_eq!(last_grapheme_start(&line), 8);
  assert_eq!(floor_grapheme_boundary(&line, 12), 8);
  assert_eq!(floor_grapheme_boundary(&line, 4), 0);
}

#[test]
fn grapheme_columns_round_trip() {
  let line = format!("{COMBINING}{THAI}x{ZWJ}");
  let base = grapheme_base_chars(&line);
  assert_eq!(base, vec!['e', 'ท', 'x', '👩']);
  assert_eq!(grapheme_column(&line, 0), 0);
  assert_eq!(grapheme_column(&line, 4), 1);
  assert_eq!(grapheme_column(&line, 12), 2);
  assert_eq!(byte_offset_of_grapheme(&line, 3), 13);
  assert_eq!(byte_offset_of_grapheme(&line, 4), line.len());
}

#[test]
fn display_columns_follow_cell_width() {
  // a 1, 中 2, Ａ (fullwidth) 2, e+acute 1, ที่ 1, 👩‍💻 2, 🇪🇸 2, क+ि 2
  let line = format!("a中Ａ{COMBINING}{THAI}{ZWJ}{FLAG}{DEVANAGARI}z");
  let starts: Vec<usize> =
    line.grapheme_indices(true).map(|(i, _)| i).collect();
  let columns: Vec<usize> =
    starts.iter().map(|&i| display_column(&line, i)).collect();
  assert_eq!(columns, vec![0, 1, 3, 5, 6, 7, 9, 11, 13]);
  // Mid-grapheme bytes report the grapheme's column.
  assert_eq!(display_column(&line, starts[5] + 4), 7);
  assert_eq!(display_column(&line, starts[6] + 4), 9);
  // And back: each column lands on the grapheme covering it.
  for (k, &col) in columns.iter().enumerate() {
    assert_eq!(byte_at_display_column(&line, col), starts[k]);
  }
  // The right half of a wide grapheme maps to its start.
  assert_eq!(byte_at_display_column(&line, 2), starts[1]);
  assert_eq!(byte_at_display_column(&line, 8), starts[5]);
  // Past the end clamps to the last grapheme.
  assert_eq!(byte_at_display_column(&line, 99), starts[8]);
  assert_eq!(byte_at_display_column("", 5), 0);
}

#[test]
fn display_width_matches_cli_justify_tabs() {
  assert_eq!(display_width("a\tb"), 10);
  assert_eq!(display_width("中文"), 4);
  assert_eq!(display_width("\u{200b}\u{200d}"), 0);
}
