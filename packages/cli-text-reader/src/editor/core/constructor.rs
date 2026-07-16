use super::{BufferState, Editor, EditorState, ViewMode};
use crate::highlights::HighlightData;
use crate::progress::generate_hash;
use arboard::Clipboard;
use cli_pdf_to_text::PdfLineKind;
use crossterm::terminal;

/// Strip terminal control characters from a document line.
///
/// The reader writes each line straight to the terminal, and a document is
/// untrusted input — the README itself suggests `curl example.com | hygg`, and
/// a plain file or EPUB can carry anything. A terminal *executes* the C0/C1
/// controls rather than printing them: ESC opens the CSI/OSC sequences that
/// repaint the screen, retitle the window, or write the clipboard (OSC 52), and
/// U+009B is an 8-bit CSI. None of that belongs in prose, so drop the whole
/// class. TAB is kept — it is legitimate layout — and no newline reaches here,
/// since lines are already split.
///
/// This is the funnel for stdin, plain-text, and EPUB content, which all arrive
/// stamped `PdfLineKind::Text`. PDF text is already sanitized by
/// cli-pdf-to-text before it gets here, and the streaming PDF path carries its
/// own line kinds (image rows are `AnsiArt`, whose escapes *are* the content) —
/// neither passes through this constructor, so nothing legitimately colored is
/// touched.
fn sanitize_document_line(line: &str) -> String {
  if line.chars().any(|c| c.is_control() && c != '\t') {
    line.chars().filter(|&c| !c.is_control() || c == '\t').collect()
  } else {
    line.to_string()
  }
}

impl Editor {
  pub fn new(lines: Vec<String>, col: usize) -> Self {
    Self::new_internal(lines, col, None)
  }

  pub fn new_with_content(
    lines: Vec<String>,
    col: usize,
    raw_content: String,
  ) -> Self {
    Self::new_internal(lines, col, Some(raw_content))
  }

  fn new_internal(
    lines: Vec<String>,
    col: usize,
    raw_content: Option<String>,
  ) -> Self {
    crate::debug::debug_log("editor", "Creating new Editor instance");

    // Neutralize terminal control sequences in the document before it is stored
    // or rendered. See sanitize_document_line: this constructor is the entry
    // for stdin/plain-text/EPUB content, which is untrusted and printed
    // verbatim.
    let lines: Vec<String> =
      lines.iter().map(|line| sanitize_document_line(line)).collect();

    // Generate hash from raw content if provided, otherwise from lines
    let document_hash = if let Some(content) = &raw_content {
      crate::debug::debug_log("editor", "Generating hash from raw content");
      generate_hash(content)
    } else {
      crate::debug::debug_log("editor", "Generating hash from justified lines");
      generate_hash(&lines)
    };

    let total_lines = lines.len();
    let (width, height) = terminal::size()
      .map(|(w, h)| (w as usize, h as usize))
      .unwrap_or((80, 24));

    // Startup narration voice + speed (env / .env / built-in defaults); the
    // `:voice` and `:speed` commands mutate these live.
    let (tts_voice, tts_speed) = crate::config::tts_settings();

    crate::debug::debug_log_state(
      "editor",
      "document_hash",
      &document_hash.to_string(),
    );
    crate::debug::debug_log_state(
      "editor",
      "total_lines",
      &total_lines.to_string(),
    );
    crate::debug::debug_log_state(
      "editor",
      "terminal_size",
      &format!("{width}x{height}"),
    );

    // Initialize clipboard - may fail on headless systems
    let clipboard = Clipboard::new().ok();
    crate::debug::debug_log_state(
      "editor",
      "clipboard_available",
      &clipboard.is_some().to_string(),
    );

    // Create initial buffer with the document
    let mut initial_buffer = BufferState::new(lines.clone());
    initial_buffer.viewport_height = height.saturating_sub(1);
    initial_buffer.viewport_start = 0;

    crate::debug::debug_log("editor", "Editor instance created successfully");

    Self {
      lines,
      line_kinds: vec![PdfLineKind::Text; total_lines],
      col,
      offset: 0,
      width,
      height,
      show_highlighter: true,
      editor_state: EditorState::new(),
      document_hash,
      total_lines,
      progress_display_until: None,
      show_progress: false,
      cursor_x: 0,
      cursor_y: height / 2,
      clipboard,
      buffers: vec![initial_buffer],
      active_buffer: 0,
      view_mode: ViewMode::Normal,
      show_cursor: true,
      last_find_char: None,
      last_find_forward: true,
      last_find_till: false,
      marks: std::collections::HashMap::new(),
      previous_position: None,
      number_prefix: String::new(),
      highlights: HighlightData::new(document_hash.to_string()),
      active_pane: 0,
      split_ratio: 0.7, // 70% for main buffer, 30% for command output
      tmux_prefix_active: false,
      needs_redraw: true,
      last_offset: 0,
      force_clear: true,
      cursor_moved: false,
      tutorial_step: 0,
      tutorial_active: false,
      tutorial_demo_mode: false,
      tutorial_start_time: None,
      demo_script: None,
      demo_action_index: 0,
      demo_saved_highlights: None,
      demo_id: None,
      demo_last_action_time: None,
      demo_hint_text: None,
      demo_hint_until: None,
      demo_typing_char_index: 0,
      demo_pending_keys: Vec::new(),
      current_tutorial_condition: None,
      tutorial_highlight_created: false,
      tutorial_yank_performed: false,
      tutorial_paste_performed: false,
      tutorial_search_navigated: false,
      tutorial_bookmark_jumped: false,
      tutorial_forward_search_used: false,
      tutorial_backward_search_used: false,
      last_executed_command: None,
      tutorial_step_completed: false,
      initial_setup_complete: false,
      last_saved_viewport_offset: 0,
      cursor_currently_visible: true,
      last_cursor_style: None,
      buffer_just_switched: false,
      pdf_streaming: None,
      pdf_restore_target: None,
      pdf_source_path: None,
      ocr_enabled: false,
      pdf_pending: None,
      pdf_load_started_at: None,
      pdf_load_finished: None,
      speech: None,
      tts_voice,
      tts_speed,
      tts_enabled: crate::config::tts_enabled_setting(),
      notes: crate::notes::NoteData::default(),
      notes_active: false,
      notes_input: String::new(),
      notes_anchor: None,
      sync: None,
      book_id: None,
      sync_mode: hygg_shared::sync::SyncMode::Full,
      sync_policy: hygg_shared::sync::AutoSyncPolicy::default(),
      auto_sync_optin: false,
      source_path: None,
      pending_server_progress: None,
      pending_server_progress_autoapply: false,
      server_progress_prompt: false,
      server_progress_scroll_at: None,
      server_progress_jump_requested_at: None,
      last_synced_offset: None,
      last_local_progress_updated_at: None,
      startup_progress_reconcile: false,
      sync_offline: false,
      reading_time_seconds: 0,
      reading_accrued: 0.0,
      reading_persisted_seconds: 0,
      reading_dirty: false,
      last_activity: std::time::Instant::now(),
      reading_last_tick: std::time::Instant::now(),
      reading_last_flush: std::time::Instant::now(),
      exit_to_home: false,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn document_lines_are_stripped_of_terminal_controls() {
    // An OSC 52 clipboard write and a CSI colour, as a hostile `curl | hygg`
    // response might carry. Removing the control bytes (ESC, BEL) is what
    // disarms the sequence: with no ESC introducer the terminal cannot act on
    // the leftover printable body, so the clipboard is never written and the
    // colour never applied — even though `]52;c;…` survives as inert text.
    let editor = Editor::new(
      vec![
        "hello\x1b]52;c;cGF5bG9hZA==\x07world".to_string(),
        "\x1b[31mred\x1b[0m text\ttabbed".to_string(),
      ],
      80,
    );
    // No ESC remains, so nothing the terminal will execute does.
    assert!(!editor.lines.iter().any(|l| l.contains('\x1b')));
    assert!(!editor.lines.iter().any(|l| l.contains('\x07')));
    // The visible characters and a real tab are preserved.
    assert!(
      editor.lines[0].contains("hello") && editor.lines[0].contains("world")
    );
    assert_eq!(editor.lines[1], "[31mred[0m text\ttabbed");
  }

  #[test]
  fn ordinary_text_is_untouched() {
    let editor = Editor::new(vec!["a normal line of prose".to_string()], 80);
    assert_eq!(editor.lines[0], "a normal line of prose");
  }
}
