use super::super::core::Editor;
use super::progress::{
  PDF_LOADING_SLOT_WIDTH, pdf_loading_slots_message_for_state,
};
use crate::editor::streaming::{LoadedPage, PageSlot, PdfStreamingState};
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
fn progress_indicator_hides_percentage_until_pdf_parser_finishes() {
  let Some(editor) = editor_with_streaming_parser_state(false) else {
    return;
  };

  assert_eq!(editor.progress_indicator_message(), " --%");
}

#[test]
fn progress_indicator_shows_percentage_after_pdf_parser_finishes() {
  let Some(editor) = editor_with_streaming_parser_state(true) else {
    return;
  };

  assert_eq!(editor.progress_indicator_message(), " 50%");
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
fn server_progress_prompt_points_to_command_not_y_hotkey() {
  let mut editor = Editor::new(vec!["line".to_string()], 80);
  editor.server_progress_prompt = true;

  let mut buffer = Vec::new();
  editor.draw_status_line_buffered(&mut buffer).unwrap();
  let output = String::from_utf8(buffer).expect("status line is utf-8");

  assert!(output.contains(":server-progress to jump"));
  assert!(!output.contains("press y"));
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
