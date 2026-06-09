use super::text_kinds;
use crate::editor::speech::{WordSpan, build_word_spans, word_byte_ranges};

#[test]
fn word_byte_ranges_handles_spaces_and_utf8() {
  assert_eq!(word_byte_ranges("the quick fox"), vec![(0, 3), (4, 9), (10, 13)]);
  // "café" is 5 bytes (é = 2); trailing word offset must be byte-based.
  let line = "café ok";
  assert_eq!(word_byte_ranges(line), vec![(0, 5), (6, 8)]);
  assert_eq!(&line[0..5], "café");
  assert_eq!(&line[6..8], "ok");
}

#[test]
fn build_word_spans_matches_persistent_offset_space() {
  // Two text lines; abs offset of line 1 must be len(line0)+1.
  let lines = vec!["the quick".to_string(), "brown fox".to_string()];
  let spans = build_word_spans(&lines, &text_kinds(2));
  assert_eq!(spans.len(), 4);

  // line 0
  assert_eq!(
    spans[0],
    WordSpan { abs_start: 0, abs_end: 3, line: 0, col_start: 0, col_end: 3 }
  );
  assert_eq!(
    spans[1],
    WordSpan { abs_start: 4, abs_end: 9, line: 0, col_start: 4, col_end: 9 }
  );
  // line 1 starts at len("the quick") + 1 = 10
  assert_eq!(
    spans[2],
    WordSpan { abs_start: 10, abs_end: 15, line: 1, col_start: 0, col_end: 5 }
  );
  assert_eq!(
    spans[3],
    WordSpan { abs_start: 16, abs_end: 19, line: 1, col_start: 6, col_end: 9 }
  );
}
