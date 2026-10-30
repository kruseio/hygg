// Golden frames for the line-highlight renderer: representative lines drawn
// through the real frame entry points, compared byte for byte. These were
// captured from the per-path renderers before they were merged into
// `line_highlights`, so they pin the visible output across the refactor; the
// cases whose output changed on purpose say so next to their expected value.

use super::super::core::{BufferState, Editor, EditorMode, ViewMode};
use super::super::highlighting::{HighlightKind, LineView};
use super::super::speech::{SpeechState, TtsStatus};
use crate::editor::char_boundary::{ByteCol, LineSpan, Pos};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

const L0: &str = "La canción está";
const L1: &str = "aquí € 漢字 ok";
const L2: &str = "plain line";
// Absolute offset of L1 in the persistent-highlight coordinate space.
const L1_ABS: usize = L0.len() + 1;

fn base() -> Editor {
  let mut editor =
    Editor::new(vec![L0.to_string(), L1.to_string(), L2.to_string()], 40);
  editor.height = 4;
  editor.offset = 0;
  editor.cursor_y = 0;
  editor.show_highlighter = true;
  editor
}

fn mode(editor: &mut Editor, mode: EditorMode) {
  editor.set_active_mode(mode);
}

// Literal, possibly stale (mid-character or past the end) selection ends.
fn select(editor: &mut Editor, start: (usize, usize), end: (usize, usize)) {
  let at = |(line, col): (usize, usize)| Pos::new(line, ByteCol::lit(col));
  let (start, end) = (at(start), at(end));
  editor.editor_state.selection_start = Some(start);
  editor.editor_state.selection_end = Some(end);
  editor.buffers[0].selection_start = Some(start);
  editor.buffers[0].selection_end = Some(end);
}

fn frame(editor: &Editor) -> String {
  let mut buffer = Vec::new();
  editor.draw_content_buffered(&mut buffer, 20, "  ").unwrap();
  String::from_utf8(buffer).unwrap()
}

fn split_frame(editor: &Editor) -> String {
  let mut buffer = Vec::new();
  editor.draw_split_view_buffered(&mut buffer, 20, "  ").unwrap();
  String::from_utf8(buffer).unwrap()
}

fn speech(current: (usize, usize)) -> SpeechState {
  SpeechState {
    rx: mpsc::channel().1,
    cancel: Arc::new(AtomicBool::new(false)),
    worker: None,
    current: Some(current),
    playing: true,
    status: Arc::new(Mutex::new(TtsStatus::Speaking)),
  }
}

fn split(editor: &mut Editor) {
  editor.height = 9;
  editor.view_mode = ViewMode::HorizontalSplit;
  let mut panel = BufferState::new(vec![L1.to_string(), L0.to_string()]);
  // The inactive pane keeps its own visual selection and search match.
  panel.mode = EditorMode::VisualChar;
  panel.selection_start = Some(Pos::new(0, ByteCol::lit(2)));
  panel.selection_end = Some(Pos::new(0, ByteCol::lit(9)));
  panel.current_match =
    Some(LineSpan::new(1, ByteCol::lit(3), ByteCol::lit(11)));
  editor.buffers.push(panel);
}

// Every case: name, frame. Built fresh each time so cases cannot leak state.
fn cases() -> Vec<(&'static str, String)> {
  let mut out = Vec::new();

  out.push(("plain", frame(&base())));

  let mut e = base();
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 3), (1, 8));
  out.push(("selection_only", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 9), (0, 3));
  out.push(("selection_multiline_reversed", frame(&e)));

  let mut e = base();
  e.editor_state.current_match =
    Some(LineSpan::new(1, ByteCol::lit(7), ByteCol::lit(10)));
  out.push(("search_only", frame(&e)));

  let mut e = base();
  e.editor_state.search_preview_active = true;
  e.editor_state.search_preview_match =
    Some(LineSpan::new(0, ByteCol::lit(3), ByteCol::lit(11)));
  e.editor_state.current_match =
    Some(LineSpan::new(1, ByteCol::lit(0), ByteCol::lit(5)));
  out.push(("search_preview_only", frame(&e)));

  let mut e = base();
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 13);
  e.highlights.add_highlight(3, 11);
  out.push(("persistent_only", frame(&e)));

  let mut e = base();
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 13);
  e.highlights.add_highlight(L1_ABS + 13, L1_ABS + 16);
  out.push(("persistent_adjacent", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 0), (1, 10));
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 16);
  out.push(("selection_then_persistent_overlap", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 7), (1, 16));
  e.highlights.add_highlight(L1_ABS, L1_ABS + 10);
  out.push(("persistent_then_selection_overlap", frame(&e)));

  let mut e = base();
  e.editor_state.current_match =
    Some(LineSpan::new(1, ByteCol::lit(0), ByteCol::lit(5)));
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 13);
  out.push(("search_and_persistent", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 0), (1, 5));
  e.editor_state.current_match =
    Some(LineSpan::new(1, ByteCol::lit(7), ByteCol::lit(13)));
  out.push(("selection_and_search", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualLine);
  select(&mut e, (0, 3), (1, 2));
  out.push(("visual_line", frame(&e)));

  let mut e = base();
  mode(&mut e, EditorMode::VisualLine);
  select(&mut e, (0, 3), (1, 2));
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 13);
  out.push(("visual_line_and_persistent", frame(&e)));

  // `V` then `:` — command mode keeps the selection visible.
  let mut e = base();
  mode(&mut e, EditorMode::Command);
  e.editor_state.visual_selection_active = true;
  e.editor_state.previous_visual_mode = Some(EditorMode::VisualLine);
  select(&mut e, (0, 3), (1, 2));
  out.push(("visual_line_in_command_mode", frame(&e)));

  // `v`, `:`, Esc — the selection is left behind in normal mode.
  let mut e = base();
  mode(&mut e, EditorMode::Normal);
  select(&mut e, (0, 3), (1, 5));
  e.highlights.add_highlight(L1_ABS + 7, L1_ABS + 13);
  out.push(("stale_selection_in_normal_mode", frame(&e)));

  let mut e = base();
  e.speech = Some(speech((3, 11)));
  out.push(("spoken_word", frame(&e)));

  let mut e = base();
  e.speech = Some(speech((3, 11)));
  e.highlights.add_highlight(12, 17);
  out.push(("spoken_word_and_persistent", frame(&e)));

  let mut e = base();
  split(&mut e);
  mode(&mut e, EditorMode::VisualChar);
  select(&mut e, (1, 0), (1, 10));
  e.highlights.add_highlight(3, 11);
  out.push(("split_active_selection_persistent", split_frame(&e)));

  let mut e = base();
  split(&mut e);
  e.editor_state.current_match =
    Some(LineSpan::new(2, ByteCol::lit(0), ByteCol::lit(5)));
  out.push(("split_active_search", split_frame(&e)));

  let mut e = base();
  split(&mut e);
  mode(&mut e, EditorMode::VisualLine);
  select(&mut e, (0, 3), (1, 2));
  out.push(("split_active_visual_line", split_frame(&e)));

  out
}

// Prints the current frames as a Rust table; used to (re)capture GOLDEN.
//   cargo test -p cli-text-reader --lib print_golden_frames -- --ignored
//   --nocapture
#[test]
#[ignore = "capture helper"]
fn print_golden_frames() {
  for (name, frame) in cases() {
    println!("  ({name:?}, {frame:?}),");
  }
}

// Captured from the pre-merge renderers; entries marked as changed show the
// unified behaviour instead (see the note on each).
#[rustfmt::skip]
const GOLDEN: &[(&str, &str)] = &[
  ("plain", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  aquí € 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("selection_only", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  aqu\u{1b}[48;5;4m\u{1b}[38;5;15mí \u{1b}[0m€ 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: a selection made backwards across lines used to light both edge
  // lines in full in the selection-only renderer (the combined and split
  // renderers already honoured the columns); now its edge columns apply.
  ("selection_multiline_reversed", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  La \u{1b}[48;5;4m\u{1b}[38;5;15mcanción está\u{1b}[0m\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí €\u{1b}[0m 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("search_only", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  aquí \u{1b}[48;5;11m\u{1b}[38;5;0m€ \u{1b}[0m漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("search_preview_only", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K\u{1b}[2;1H  aquí € 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("persistent_only", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K\u{1b}[2;1H  aquí \u{1b}[48;5;11m\u{1b}[38;5;0m€ 漢\u{1b}[0m字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("persistent_adjacent", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  aquí \u{1b}[48;5;11m\u{1b}[38;5;0m€ 漢字\u{1b}[0m ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("selection_then_persistent_overlap", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € \u{1b}[0m\u{1b}[48;5;11m\u{1b}[38;5;0m漢字\u{1b}[0m ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: where selection overlaps a persistent highlight the selection now
  // wins; the old combined renderer let whichever range started first win.
  ("persistent_then_selection_overlap", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  \u{1b}[48;5;11m\u{1b}[38;5;0maquí \u{1b}[0m\u{1b}[48;5;4m\u{1b}[38;5;15m€ 漢字\u{1b}[0m ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: the search match is drawn on a line with persistent highlights; the
  // old combined renderer dropped it.
  ("search_and_persistent", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  \u{1b}[48;5;11m\u{1b}[38;5;0maquí\u{1b}[0m \u{1b}[48;5;11m\u{1b}[38;5;0m€ 漢\u{1b}[0m字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: a search match outside the selection stays visible; the old
  // selection-only renderer dropped it.
  ("selection_and_search", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí\u{1b}[0m \u{1b}[48;5;11m\u{1b}[38;5;0m€ 漢\u{1b}[0m字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("visual_line", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  \u{1b}[48;5;4m\u{1b}[38;5;15mLa canción está\u{1b}[0m\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € 漢字 ok\u{1b}[0m\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Bytes only: the old renderer emitted an empty styled segment here.
  ("visual_line_and_persistent", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  \u{1b}[48;5;4m\u{1b}[38;5;15mLa canción está\u{1b}[0m\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € 漢字 ok\u{1b}[0m\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: `V` then `:` keeps whole lines selected, as the combined renderer
  // already did; the selection-only renderer fell back to the columns.
  ("visual_line_in_command_mode", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  \u{1b}[48;5;4m\u{1b}[38;5;15mLa canción está\u{1b}[0m\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € 漢字 ok\u{1b}[0m\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: a selection left behind outside visual mode is hidden on every
  // line; it used to show only on lines with persistent highlights.
  ("stale_selection_in_normal_mode", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m\u{1b}[38;5;15m  La canción está\u{1b}[0m\u{1b}[2;1H  aquí \u{1b}[48;5;11m\u{1b}[38;5;0m€ 漢\u{1b}[0m字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  ("spoken_word", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  La \u{1b}[48;5;14m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K\u{1b}[2;1H  aquí € 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Changed: other highlights on the spoken line stay visible around the word;
  // the spoken-word renderer used to draw only the word.
  ("spoken_word_and_persistent", "\u{1b}[1;1H\u{1b}[1;1H\u{1b}[48;2;40;40;40m                    \u{1b}[1;1H\u{1b}[0m  La \u{1b}[48;5;14m\u{1b}[38;5;0mcanción\u{1b}[0m \u{1b}[48;5;11m\u{1b}[38;5;0mestá\u{1b}[0m\u{1b}[K\u{1b}[2;1H  aquí € 漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[49m"),
  // Bytes only: one Clear instead of the old duplicated one.
  ("split_active_selection_persistent", "\u{1b}[1;1H  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € \u{1b}[0m漢字 ok\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[4;1H\u{1b}[2K\u{1b}[5;1H\u{1b}[2K\u{1b}[6;1H\u{1b}[38;5;8m────────────────────\u{1b}[0m\u{1b}[7;1H  aq\u{1b}[48;5;4m\u{1b}[38;5;15muí €\u{1b}[0m 漢字 ok\u{1b}[K\u{1b}[8;1H  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K"),
  ("split_active_search", "\u{1b}[1;1H  La canción está\u{1b}[K\u{1b}[2;1H  aquí € 漢字 ok\u{1b}[K\u{1b}[3;1H  \u{1b}[48;5;11m\u{1b}[38;5;0mplain\u{1b}[0m line\u{1b}[K\u{1b}[4;1H\u{1b}[2K\u{1b}[5;1H\u{1b}[2K\u{1b}[6;1H\u{1b}[38;5;8m────────────────────\u{1b}[0m\u{1b}[7;1H  aq\u{1b}[48;5;4m\u{1b}[38;5;15muí €\u{1b}[0m 漢字 ok\u{1b}[K\u{1b}[8;1H  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K"),
  ("split_active_visual_line", "\u{1b}[1;1H  \u{1b}[48;5;4m\u{1b}[38;5;15mLa canción está\u{1b}[0m\u{1b}[K\u{1b}[2;1H  \u{1b}[48;5;4m\u{1b}[38;5;15maquí € 漢字 ok\u{1b}[0m\u{1b}[K\u{1b}[3;1H  plain line\u{1b}[K\u{1b}[4;1H\u{1b}[2K\u{1b}[5;1H\u{1b}[2K\u{1b}[6;1H\u{1b}[38;5;8m────────────────────\u{1b}[0m\u{1b}[7;1H  aq\u{1b}[48;5;4m\u{1b}[38;5;15muí €\u{1b}[0m 漢字 ok\u{1b}[K\u{1b}[8;1H  La \u{1b}[48;5;11m\u{1b}[38;5;0mcanción\u{1b}[0m está\u{1b}[K"),
];

#[test]
fn frames_match_golden_output() {
  let cases = cases();
  assert_eq!(cases.len(), GOLDEN.len(), "every case needs a golden frame");
  for ((name, frame), (golden_name, golden)) in cases.iter().zip(GOLDEN) {
    assert_eq!(name, golden_name);
    assert_eq!(frame, golden, "frame {name:?} changed");
  }
}

// The main document's pane, while the other pane is active, draws its own
// stored selection — not the active buffer's live one, which the old split
// combined renderer borrowed for any line with a persistent highlight.
#[test]
fn inactive_main_pane_uses_its_own_selection() {
  let mut e = base();
  split(&mut e);
  e.highlights.add_highlight(3, 11);
  e.buffers[0].mode = EditorMode::Normal;
  e.buffers[0].selection_start = None;
  e.buffers[0].selection_end = None;
  // The active (bottom) pane's live selection, on its own line 0.
  e.active_buffer = 1;
  e.active_pane = 1;
  mode(&mut e, EditorMode::VisualChar);
  e.editor_state.selection_start = Some(Pos::new(0, ByteCol::lit(0)));
  e.editor_state.selection_end = Some(Pos::new(0, ByteCol::lit(5)));

  let main = e.line_highlights(LineView::Pane(0), 0, L0).unwrap();
  assert_eq!(
    main,
    vec![(ByteCol::lit(3), ByteCol::lit(11), HighlightKind::Persistent)]
  );
  let active = e.line_highlights(LineView::Pane(1), 0, L1).unwrap();
  assert_eq!(
    active,
    vec![(ByteCol::lit(0), ByteCol::lit(5), HighlightKind::Selection)]
  );
}
