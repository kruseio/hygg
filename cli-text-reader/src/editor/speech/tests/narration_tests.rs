use std::time::Duration;

use cli_pdf_to_text::PdfLineKind;

use crate::editor::core::Editor;

// Regression: narrating from a reading position that has no narratable word
// at or after it (cursor on ASCII art, a blank/placeholder row, or past the
// last text line — all common on a streaming PDF whose lower pages haven't
// rendered text yet) must NOT restart narration from the top of the
// document. Doing so dragged the reading cursor to line 0, and since the
// reading position is what gets persisted as progress, re-opening the
// document then resumed at the very beginning.
#[test]
fn narration_with_no_following_word_does_not_jump_to_top() {
  // High text, then a lower region of art rows that produce no word spans.
  let mut lines: Vec<String> =
    (0..300).map(|i| format!("line {i} has several real words here")).collect();
  lines.extend((300..600).map(|_| "\u{2580}\u{2580}\u{2580}".to_string()));
  let mut kinds = vec![PdfLineKind::Text; 300];
  kinds.extend(vec![PdfLineKind::AnsiArt; 300]);

  let mut editor = Editor::new(lines, 80);
  editor.height = 24;
  editor.total_lines = 600;
  editor.line_kinds = kinds;
  // Park the cursor deep, in the art region — no word span at/after line 411.
  editor.offset = 400;
  editor.cursor_y = 11;
  let start_line = editor.offset + editor.cursor_y;

  editor.start_narration();
  // Give any (incorrectly) spawned worker time to emit its first word event,
  // then drain + recenter exactly like the main loop does.
  std::thread::sleep(Duration::from_millis(80));
  editor.drain_speech();
  editor.center_cursor();
  let after = editor.offset + editor.cursor_y;
  editor.stop_narration();

  // Nothing narratable from here, so narration must be a no-op and the
  // reading position must stay put (never collapse toward the top).
  assert!(
    editor.speech.is_none(),
    "narration should not start with no following word"
  );
  assert_eq!(
    after, start_line,
    "reading position must not move (was {start_line}, became {after})"
  );
}

// The complementary case: when there IS a word at/after the cursor, narration
// starts there and the reading line tracks forward (never jumps to the top).
#[test]
fn narration_from_deep_cursor_with_following_text_stays_forward() {
  let lines: Vec<String> =
    (0..600).map(|i| format!("line {i} has several words here")).collect();
  let mut editor = Editor::new(lines, 80);
  editor.height = 24;
  editor.total_lines = 600;
  editor.offset = 400;
  editor.cursor_y = 11; // reading line 411
  let start_line = editor.offset + editor.cursor_y;

  editor.start_narration();
  assert!(
    editor.speech.is_some(),
    "narration should start (text follows cursor)"
  );

  let mut min_line = start_line;
  for _ in 0..16 {
    std::thread::sleep(Duration::from_millis(60));
    editor.drain_speech();
    editor.center_cursor();
    min_line = min_line.min(editor.offset + editor.cursor_y);
    if !editor.is_narrating() {
      break;
    }
  }
  editor.stop_narration();
  assert!(
    min_line >= start_line.saturating_sub(1),
    "narration reading line moved backward toward the top: min={min_line} start={start_line}"
  );
}
