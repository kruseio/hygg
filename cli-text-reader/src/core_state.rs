use arboard::Clipboard;
use cli_pdf_to_text::PdfLineKind;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::KeyEvent;
use std::collections::HashMap;
use std::time::Instant;

use super::core_types::{BufferState, EditorState, ViewMode};
use crate::demo_script::DemoScript;
use crate::editor::speech::SpeechState;
use crate::editor::streaming::{PdfStreamingState, PendingPdfStream};
use crate::highlights::HighlightData;
use crate::interactive_tutorial_buffer::TutorialSuccessCondition;

pub struct Editor {
  pub lines: Vec<String>,
  pub line_kinds: Vec<PdfLineKind>,
  pub col: usize,
  pub offset: usize,
  pub width: usize,
  pub height: usize,
  pub show_highlighter: bool,
  pub editor_state: EditorState,
  pub document_hash: u64,
  pub total_lines: usize,
  #[allow(dead_code)]
  pub progress_display_until: Option<Instant>,
  pub show_progress: bool,
  pub cursor_x: usize,
  pub cursor_y: usize,
  pub clipboard: Option<Clipboard>,
  pub buffers: Vec<BufferState>,
  pub active_buffer: usize,
  pub view_mode: ViewMode,
  pub show_cursor: bool,
  pub last_find_char: Option<char>,
  pub last_find_forward: bool,
  pub last_find_till: bool,
  pub marks: HashMap<char, (usize, usize)>,
  pub previous_position: Option<(usize, usize)>,
  pub number_prefix: String,
  pub highlights: HighlightData,
  // Split view management
  pub active_pane: usize, // 0 = top pane, 1 = bottom pane
  pub split_ratio: f32,   // Percentage for top pane (0.0-1.0)
  pub tmux_prefix_active: bool, // Tracks if Ctrl+B was pressed
  // Rendering optimization
  pub needs_redraw: bool,
  pub last_offset: usize,
  pub force_clear: bool,
  pub cursor_moved: bool,
  // Tutorial state
  pub tutorial_step: usize,
  pub tutorial_active: bool,
  pub tutorial_demo_mode: bool,
  pub tutorial_start_time: Option<Instant>,
  // Demo script execution
  pub demo_script: Option<DemoScript>,
  pub demo_action_index: usize,
  pub demo_id: Option<usize>,
  pub demo_last_action_time: Option<Instant>,
  pub demo_hint_text: Option<String>,
  pub demo_hint_until: Option<Instant>,
  pub demo_typing_char_index: usize,
  pub demo_pending_keys: Vec<KeyEvent>,
  pub current_tutorial_condition: Option<TutorialSuccessCondition>,
  pub tutorial_highlight_created: bool,
  pub tutorial_yank_performed: bool,
  pub tutorial_paste_performed: bool,
  pub tutorial_search_navigated: bool,
  pub tutorial_bookmark_jumped: bool,
  pub tutorial_forward_search_used: bool,
  pub tutorial_backward_search_used: bool,
  pub last_executed_command: Option<String>,
  pub tutorial_step_completed: bool,
  // Track if initial setup is complete to avoid resize issues
  pub initial_setup_complete: bool,
  // Track last saved viewport offset to avoid duplicate saves
  pub last_saved_viewport_offset: usize,
  // Track cursor visibility state to optimize hide/show operations
  pub cursor_currently_visible: bool,
  // Track cursor style state to avoid redundant terminal style changes
  pub last_cursor_style: Option<SetCursorStyle>,
  // Track if we just switched buffers to skip centering
  pub buffer_just_switched: bool,
  // Streaming-PDF page table; None for non-streaming sessions.
  pub pdf_streaming: Option<PdfStreamingState>,
  pub pdf_source_path: Option<String>,
  pub ocr_enabled: bool,
  // Background-opening PDF state; held while we wait for the doc to parse.
  pub pdf_pending: Option<PendingPdfStream>,
  pub pdf_load_started_at: Option<(Instant, String)>,
  pub pdf_load_finished: Option<(Instant, f32, String)>,
  // TTS narration state (Phase 1: fake voice driving highlight + auto-scroll).
  pub speech: Option<SpeechState>,
  // Live narration voice id + speed used by `:speak`. Seeded from
  // TTS_VOICE/TTS_SPEED (env or ~/.config/hygg/.env) or built-in defaults, and
  // changed at runtime by the `:voice` / `:speed` commands.
  pub tts_voice: String,
  pub tts_speed: f32,
}
