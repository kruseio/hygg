// Issue #99: renderers slice lines with byte columns, and a column that is out
// of range or inside a multi-byte character used to panic the redraw. These
// tests feed every renderer every byte offset — on boundaries, inside
// characters and past the end — and require a frame that is valid UTF-8.

use crate::editor::char_boundary::{ByteCol, CursorCol, LineSpan, Pos};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crossterm::QueueableCommand;
use crossterm::cursor::MoveTo;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::core::{BufferState, Editor, EditorMode, ViewMode};
use super::super::highlighting::LineView;
use super::super::speech::{SpeechState, TtsStatus};

// Spanish accents (2 bytes), a 3-byte '€' and 3-byte CJK characters.
pub(super) const LINE: &str = "¿Qué? La canción está aquí: 5 € y 漢字";
const SECOND: &str = "Él añadió más 漢 ñ";

pub(super) fn key(c: char) -> KeyEvent {
  KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

pub(super) fn utf8(buffer: Vec<u8>) -> String {
  String::from_utf8(buffer).expect("rendered frame should be valid UTF-8")
}

// Every byte offset of the line, including mid-character and past-the-end.
fn offsets(line: &str) -> std::ops::RangeInclusive<usize> {
  0..=line.len() + 2
}

pub(super) fn editor() -> Editor {
  let mut editor = Editor::new(vec![LINE.to_string(), SECOND.to_string()], 80);
  editor.height = 5;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor
}

fn set_visual_char(editor: &mut Editor) {
  editor.editor_state.mode = EditorMode::VisualChar;
  editor.buffers[0].mode = EditorMode::VisualChar;
}

// Literal, possibly stale (mid-character or past the end) selection ends.
fn select(editor: &mut Editor, start: (usize, usize), end: (usize, usize)) {
  let at = |(line, col): (usize, usize)| Pos::new(line, ByteCol::lit(col));
  let (start, end) = (at(start), at(end));
  editor.editor_state.selection_start = Some(start);
  editor.editor_state.selection_end = Some(end);
}

// Two persistent highlights on the second line, overlapping, both expressed in
// absolute document offsets as the highlight store keeps them.
fn highlight_second_line(editor: &mut Editor, start: usize, end: usize) {
  let base = LINE.len() + 1;
  editor.highlights.highlights.clear();
  editor.highlights.add_highlight(base + start, base + end);
  editor
    .highlights
    .add_highlight(base + start / 2, base + end.saturating_sub(1));
}

// Render one line through the shared highlighter, checking the ranges it
// resolved: sorted, disjoint, inside the line and on char boundaries.
fn render(editor: &Editor, view: LineView, line_idx: usize, line: &str) {
  if let Some(ranges) = editor.line_highlights(view, line_idx, line) {
    let mut pos = 0;
    for &(start, end, _) in &ranges {
      let (start, end) = (start.raw(), end.raw());
      assert!(pos <= start && start < end && end <= line.len(), "{ranges:?}");
      assert!(line.is_char_boundary(start) && line.is_char_boundary(end));
      pos = end;
    }
  }
  let mut buffer = Vec::new();
  editor
    .render_line_highlights(&mut buffer, view, line_idx, line, "  ")
    .unwrap();
  utf8(buffer);
}

#[test]
fn selection_survives_every_byte_offset() {
  let mut editor = editor();
  for mode in [EditorMode::VisualChar, EditorMode::VisualLine] {
    editor.set_active_mode(mode);
    for start in offsets(LINE) {
      for end in offsets(LINE) {
        // Single-line selection, both orders.
        select(&mut editor, (0, start), (0, end));
        render(&editor, LineView::Main, 0, LINE);

        // Multi-line selection: first line from `start`, last line to `end`.
        select(&mut editor, (0, start), (1, end));
        render(&editor, LineView::Main, 0, LINE);
        render(&editor, LineView::Main, 1, SECOND);
      }
    }
  }
}

#[test]
fn search_match_survives_every_byte_offset() {
  let mut editor = editor();
  for start in offsets(LINE) {
    for end in offsets(LINE) {
      for preview in [false, true] {
        editor.editor_state.search_preview_active = preview;
        editor.editor_state.search_preview_match =
          Some(LineSpan::new(0, ByteCol::lit(start), ByteCol::lit(end)));
        editor.editor_state.current_match =
          Some(LineSpan::new(0, ByteCol::lit(start), ByteCol::lit(end)));
        // A match on the line claims it even when it is empty.
        assert!(editor.line_highlights(LineView::Main, 0, LINE).is_some());
        render(&editor, LineView::Main, 0, LINE);
      }
    }
  }
}

#[test]
fn persistent_and_overlapping_highlights_survive_every_byte_offset() {
  let mut editor = editor();
  set_visual_char(&mut editor);
  for start in offsets(SECOND) {
    for end in offsets(SECOND) {
      highlight_second_line(&mut editor, start, end);
      // A selection crossing into the second line at a different offset, and
      // a search match, all overlapping the persistent ranges.
      select(&mut editor, (0, end), (1, start));
      editor.editor_state.current_match =
        Some(LineSpan::new(1, ByteCol::lit(end / 2), ByteCol::lit(start + 3)));
      render(&editor, LineView::Main, 0, LINE);
      render(&editor, LineView::Main, 1, SECOND);
      // Single-line selection overlapping the persistent highlight.
      select(&mut editor, (1, end), (1, start));
      render(&editor, LineView::Main, 1, SECOND);
    }
  }
}

// Persistent offsets recorded against a longer line (before a reflow, say) are
// rendered against the shorter line actually on screen.
#[test]
fn highlights_survive_offsets_from_a_longer_line() {
  let mut editor = editor();
  set_visual_char(&mut editor);
  let short = "ñó€";
  for start in offsets(SECOND) {
    for end in offsets(SECOND) {
      highlight_second_line(&mut editor, start, end);
      select(&mut editor, (1, start), (1, end));
      editor.editor_state.current_match =
        Some(LineSpan::new(1, ByteCol::lit(start), ByteCol::lit(end)));
      render(&editor, LineView::Main, 1, short);
      render(&editor, LineView::Pane(0), 1, short);
    }
  }
}

#[test]
fn spoken_word_survives_every_byte_offset() {
  let mut editor = editor();
  editor.highlights.add_highlight(2, 9);
  for start in offsets(LINE) {
    for end in offsets(LINE) {
      editor.speech = Some(SpeechState {
        rx: mpsc::channel().1,
        cancel: Arc::new(AtomicBool::new(false)),
        worker: None,
        current: Some((start, end)),
        playing: true,
        status: Arc::new(Mutex::new(TtsStatus::Speaking)),
      });
      render(&editor, LineView::Main, 0, LINE);
      // A shorter line than the one the word offsets were computed against.
      render(&editor, LineView::Main, 0, "ñó");
    }
  }
}

#[test]
fn full_frame_survives_every_byte_offset() {
  let mut editor = editor();
  editor.show_highlighter = true;
  set_visual_char(&mut editor);
  for start in offsets(LINE) {
    for end in offsets(LINE) {
      select(&mut editor, (0, start), (0, end));
      editor.editor_state.current_match =
        Some(LineSpan::new(0, ByteCol::lit(end), ByteCol::lit(start)));
      highlight_second_line(&mut editor, start.min(SECOND.len()), end);
      let mut buffer = Vec::new();
      editor.draw_content(&mut buffer, 80, "  ").unwrap();
      // Selection and search on the persistent-highlight line too.
      select(&mut editor, (1, start), (1, end));
      editor.editor_state.current_match =
        Some(LineSpan::new(1, ByteCol::lit(start), ByteCol::lit(end)));
      editor.draw_content(&mut buffer, 80, "  ").unwrap();
      utf8(buffer);
    }
  }
}

fn split_editor() -> Editor {
  let mut editor = editor();
  editor.view_mode = ViewMode::HorizontalSplit;
  editor.buffers[0].lines = editor.lines.clone();
  editor.buffers[0].line_kinds = editor.line_kinds.clone();
  let mut panel = BufferState::new(vec![LINE.to_string(), SECOND.to_string()]);
  panel.mode = EditorMode::VisualChar;
  editor.buffers.push(panel);
  editor.active_buffer = 0;
  editor.active_pane = 0;
  editor.height = 9;
  editor
}

#[test]
fn split_view_survives_every_byte_offset() {
  let mut editor = split_editor();
  set_visual_char(&mut editor);
  for start in offsets(LINE) {
    for end in offsets(LINE) {
      // Active (main) buffer: selection, search and persistent highlights.
      select(&mut editor, (0, start), (1, end));
      editor.editor_state.current_match =
        Some(LineSpan::new(0, ByteCol::lit(start), ByteCol::lit(end)));
      highlight_second_line(&mut editor, start.min(SECOND.len()), end);
      // Inactive pane: a selection and search match of its own.
      editor.buffers[1].selection_start = Some(Pos::new(0, ByteCol::lit(end)));
      editor.buffers[1].selection_end = Some(Pos::new(0, ByteCol::lit(start)));
      editor.buffers[1].current_match =
        Some(LineSpan::new(1, ByteCol::lit(start), ByteCol::lit(end)));

      for (line_idx, line) in [LINE, SECOND].into_iter().enumerate() {
        for buffer_idx in [0, 1] {
          render(&editor, LineView::Pane(buffer_idx), line_idx, line);
        }
      }
      let mut buffer = Vec::new();
      editor.draw_split_view(&mut buffer, 80, "").unwrap();
      utf8(buffer);
    }
  }
}

// Valid, on-boundary columns still highlight exactly the selected text.
#[test]
fn valid_selection_renders_the_selected_text() {
  let mut editor = editor();
  set_visual_char(&mut editor);
  let start = LINE.find("canción").unwrap();
  let end = start + "canción".len();
  select(&mut editor, (0, start), (0, end));
  let mut buffer = Vec::new();
  editor
    .render_line_highlights(&mut buffer, LineView::Main, 0, LINE, "")
    .unwrap();
  let out = utf8(buffer);
  assert!(out.starts_with("¿Qué? La "), "got {out:?}");
  assert!(out.contains("canción\x1b[0m"), "got {out:?}");
  assert!(out.contains(" está aquí: 5 € y 漢字"), "got {out:?}");
}
