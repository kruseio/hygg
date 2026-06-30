use super::super::core::Editor;
use super::progress::{
  PDF_LOADING_SLOT_WIDTH, pdf_loading_slots_message_for_state,
};
use crate::editor::streaming::{
  LoadedPage, PageSlot, PdfStreamingState, PendingPdfStream,
};
use cli_pdf_to_text::PdfStream;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};

fn editor_with_streaming_parser_state(fully_loaded: bool) -> Option<Editor> {
  let pdf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../test-data/pdf/progit-1-50.pdf");
  if !pdf_path.exists() {
    return None;
  }
  let stream = Arc::new(
    PdfStream::open(pdf_path.to_str().expect("utf-8 path"))
      .expect("PdfStream should open valid test PDF"),
  );
  let (_tx, rx) = mpsc::channel();
  let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
  editor.offset = 49;
  editor.cursor_y = 0;
  editor.total_lines = 100;
  editor.pdf_streaming = Some(PdfStreamingState {
    stream,
    col: 80,
    pages: if fully_loaded {
      vec![PageSlot::Loaded(LoadedPage::from_raw(
        "loaded page".to_string(),
        80,
      ))]
    } else {
      vec![
        PageSlot::Loaded(LoadedPage::from_raw("loaded page".to_string(), 80)),
        PageSlot::Loading,
      ]
    },
    receiver: rx,
    cancel: Arc::new(AtomicBool::new(false)),
    fully_loaded,
    ocr_loading: false,
    ocr_receiver: None,
    ocr_cancel: None,
    ocr_worker: None,
    worker: None,
  });
  Some(editor)
}

#[test]
fn pdf_loading_slots_keep_fixed_width_when_inactive() {
  let message = pdf_loading_slots_message_for_state(false, false, false, 0);

  assert_eq!(message, " ".repeat(PDF_LOADING_SLOT_WIDTH));
  assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
}

#[test]
fn pdf_loading_slots_hide_completed_parser_slot() {
  let message = pdf_loading_slots_message_for_state(false, true, false, 0);

  assert_eq!(message, "     O[◲]     ");
  assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
}

#[test]
fn pdf_loading_slots_hide_completed_ocr_slot() {
  let message = pdf_loading_slots_message_for_state(true, false, false, 0);

  assert_eq!(message, "P[◰]          ");
  assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
}

#[test]
fn pdf_loading_slots_show_tts_spinner() {
  let message = pdf_loading_slots_message_for_state(false, false, true, 0);

  assert_eq!(message, "          T[◳]");
  assert!(message.contains("T[◳]"));
  assert_eq!(message.chars().count(), PDF_LOADING_SLOT_WIDTH);
}

#[test]
fn progress_indicator_shows_page_with_pending_percentage_while_loading() {
  let Some(mut editor) = editor_with_streaming_parser_state(false) else {
    return;
  };
  editor.show_page_numbers = true;

  // While pages are still streaming the page counter is already known, so it
  // is shown up front; only the line-based percentage is held at `--`.
  assert_eq!(editor.progress_indicator_message(), "2/2 (--%)");
}

#[test]
fn progress_indicator_shows_page_and_percentage_for_pdf() {
  let Some(mut editor) = editor_with_streaming_parser_state(true) else {
    return;
  };
  editor.show_page_numbers = true;

  // The fully-loaded fixture has a single page, so the page counter reads
  // `1/1` while the percentage still reflects the line-based reading position.
  assert_eq!(editor.progress_indicator_message(), "1/1 (50%)");
}

#[test]
fn progress_indicator_hides_pages_when_disabled() {
  // Page numbers default to off, so even a fully-loaded PDF shows only the
  // percentage until the reader enables them with `:pagenumbers`.
  let Some(editor) = editor_with_streaming_parser_state(true) else {
    return;
  };
  assert!(!editor.show_page_numbers);

  assert_eq!(editor.progress_indicator_message(), "50%");
}

#[test]
fn progress_indicator_shows_percentage_only_without_pages() {
  // No PDF streaming state means no physical page structure (EPUB, plain
  // text, …), so only the percentage is shown even with the toggle on.
  let mut editor = Editor::new(vec!["line".to_string(); 100], 80);
  editor.show_page_numbers = true;
  editor.offset = 49;
  editor.cursor_y = 0;
  editor.total_lines = 100;

  assert_eq!(editor.progress_indicator_message(), "50%");
}

#[test]
fn progress_indicator_position_is_stable_across_load_completion() {
  let Some(mut editor) = editor_with_streaming_parser_state(true) else {
    return;
  };
  editor.show_page_numbers = true;

  // The reserved slot depends only on the (fixed) total page count, so the
  // indicator's start column — and the loading spinners packed beside it —
  // must not move when streaming finishes and the percentage fills in.
  let loaded = editor.progress_indicator_slot_width();
  editor.pdf_streaming.as_mut().expect("streaming state").fully_loaded = false;
  let loading = editor.progress_indicator_slot_width();

  assert_eq!(loaded, loading);
  // And the rendered text never exceeds the reserved slot, so it can be
  // right-aligned without shifting.
  assert!(editor.progress_indicator_message().chars().count() <= loaded);
}

#[test]
fn page_slot_widens_only_when_page_numbers_enabled() {
  let Some(mut editor) = editor_with_streaming_parser_state(true) else {
    return;
  };

  // Disabled (the default): the bar reserves just the percentage slot, so it
  // does not account for page numbers.
  assert!(!editor.show_page_numbers);
  let percentage_only = editor.progress_indicator_slot_width();
  assert_eq!(percentage_only, "100%".len());

  // Enabled: the reservation grows to fit a (digit-floored) page counter.
  editor.show_page_numbers = true;
  assert!(editor.progress_indicator_slot_width() > percentage_only);
}

#[test]
fn page_slot_reserved_consistently_from_open_through_load() {
  // Page numbers on, PDF still opening: `pdf_pending` is set but no page table
  // exists yet. The slot must already reserve page-counter width so nothing
  // shifts the instant streaming begins.
  let mut opening = Editor::new(vec!["opening".to_string()], 80);
  opening.show_page_numbers = true;
  let (_tx, rx) = mpsc::channel();
  opening.pdf_pending = Some(PendingPdfStream {
    receiver: rx,
    started_at: std::time::Instant::now(),
    canonical_path_display: "doc.pdf".to_string(),
    restore_line_in_page: None,
    restore_cursor_y: None,
  });
  let opening_slot = opening.progress_indicator_slot_width();
  assert!(opening_slot > "100%".len());

  // A typical PDF (page count under the digit floor) reserves exactly the same
  // width once streaming, so the indicator does not jump from the open splash.
  let Some(mut streaming) = editor_with_streaming_parser_state(false) else {
    return;
  };
  streaming.show_page_numbers = true;
  assert_eq!(streaming.progress_indicator_slot_width(), opening_slot);
}

#[test]
fn buffered_status_draws_ocr_slot_when_progress_is_disabled() {
  let Some(mut editor) = editor_with_streaming_parser_state(true) else {
    return;
  };
  editor.show_progress = false;
  editor.pdf_streaming.as_mut().expect("streaming state").ocr_loading = true;

  let mut buffer = Vec::new();
  editor.draw_status_line_buffered(&mut buffer).unwrap();
  let output = String::from_utf8(buffer).expect("status line is utf-8");

  assert!(output.contains("O["));
}

#[test]
fn command_completion_text_uses_remaining_status_line_width() {
  let mut editor = Editor::new(vec!["line".to_string()], 80);
  editor.width = 14;
  editor.editor_state.command_completion = Some("about author".to_string());

  assert_eq!(editor.command_completion_text(3).as_deref(), Some("  about aut"));
}

#[test]
fn command_completion_text_hides_when_command_uses_line() {
  let mut editor = Editor::new(vec!["line".to_string()], 80);
  editor.width = 4;
  editor.editor_state.command_completion = Some("about author".to_string());

  assert!(editor.command_completion_text(3).is_none());
}

// --- Status-bar layout primitives -----------------------------------------

use super::layout::{StatusSlot, draw_right_anchored, pack_right_anchored};

/// Minimal `StatusSlot` for exercising the layout engine independently of the
/// editor's concrete elements.
struct FixedSlot {
  width: usize,
  text: Option<&'static str>,
}

impl StatusSlot for FixedSlot {
  fn reserved_width(&self) -> usize {
    self.width
  }
  fn render(&self) -> Option<String> {
    self.text.map(str::to_string)
  }
}

#[test]
fn pack_right_anchored_packs_slots_leftward_with_gap() {
  // width 40, margin 2, gap 1, slots [5, 4] given rightmost-first.
  let starts = pack_right_anchored(40, 2, 1, &[5, 4]);
  // Rightmost: 40 - 2 - 5 = 33. Next: 33 - 1 - 4 = 28.
  assert_eq!(starts, vec![33, 28]);
}

#[test]
fn pack_right_anchored_saturates_on_narrow_terminal() {
  assert_eq!(pack_right_anchored(4, 2, 1, &[5, 4]), vec![0, 0]);
}

#[test]
fn draw_right_anchored_right_aligns_within_reserved_width() {
  let progress = FixedSlot { width: 5, text: Some("9%") };
  let spinner = FixedSlot { width: 4, text: Some("P[.]") };
  let slots: [&dyn StatusSlot; 2] = [&progress, &spinner];

  let mut buf = Vec::new();
  draw_right_anchored(&mut buf, 40, 0, 2, 1, &slots).unwrap();
  let out = String::from_utf8(buf).unwrap();

  // "9%" right-aligned in 5 columns, plus the spinner verbatim.
  assert!(out.contains("   9%"), "got: {out:?}");
  assert!(out.contains("P[.]"), "got: {out:?}");
}

#[test]
fn unrendered_slot_still_reserves_its_neighbours_column() {
  fn spinner_output(progress_text: Option<&'static str>) -> String {
    let progress = FixedSlot { width: 5, text: progress_text };
    let spinner = FixedSlot { width: 4, text: Some("P[.]") };
    let slots: [&dyn StatusSlot; 2] = [&progress, &spinner];
    let mut buf = Vec::new();
    draw_right_anchored(&mut buf, 40, 0, 2, 1, &slots).unwrap();
    String::from_utf8(buf).unwrap()
  }

  // The spinner lands at the same column (MoveTo to col 28 => CSI `1;29H`)
  // whether or not the rightmost slot draws — reservation, not contents,
  // fixes the layout.
  let shown = spinner_output(Some("9%"));
  let hidden = spinner_output(None);
  assert!(shown.contains("\u{1b}[1;29H"), "got: {shown:?}");
  assert!(hidden.contains("\u{1b}[1;29H"), "got: {hidden:?}");
  assert!(hidden.contains("P[.]"));
}
