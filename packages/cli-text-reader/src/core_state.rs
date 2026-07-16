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
use crate::notes::NoteData;

/// Saved resume position for a streaming PDF, kept *unresolved* until the
/// target page has real content. `word_offset` (the page-local non-whitespace
/// character anchor) is the exact, width-independent row and wins when
/// present; `line_in_page` is the unclamped saved row used as the fallback for
/// anchors that can't resolve (image rows, older saves without an anchor).
/// Resolving either against a placeholder's own lines would corrupt the
/// position, which is why resolution waits for the page to load.
#[derive(Clone, Copy, Debug)]
pub struct PdfRestoreTarget {
  pub page: u32,
  pub line_in_page: usize,
  pub cursor_y: Option<usize>,
  pub word_offset: Option<usize>,
}

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
  // The document's real highlights, snapshotted when a demo starts. The demo
  // drives `:h` on the user's own document and its cleanup used to clear every
  // highlight for that document from disk — so a marketing demo run over a
  // real book erased that book's highlights. Restored verbatim on
  // completion.
  pub demo_saved_highlights: Option<Vec<crate::highlights::Highlight>>,
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
  // Pending resume target for a streaming PDF. Held until the target page has
  // *real* rendered content so the saved row isn't clamped against — and the
  // word anchor isn't resolved against — a 1-line placeholder (either would
  // lose the position when the page hasn't preloaded, e.g. bundled-OCR opens
  // with preload radius 0). Applied by `apply_pdf_restore_target_if_ready` and
  // cleared on the first user keypress or a server-progress jump.
  pub pdf_restore_target: Option<PdfRestoreTarget>,
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
  // Master TTS on/off switch, seeded from ENABLE_TTS (config / `--tts`). When
  // false, `:speak` and the narration hotkey are inert.
  pub tts_enabled: bool,
  // Per-document notes (`:note`), loaded at startup and persisted locally.
  pub notes: NoteData,
  // True while the `:note` overlay is capturing keystrokes as note text.
  pub notes_active: bool,
  // The note currently being typed in the notes overlay.
  pub notes_input: String,
  // Document line the notes overlay was opened on, attached to new notes.
  pub notes_anchor: Option<usize>,
  // Background sync engine handle; None when no server is configured
  // (offline).
  pub sync: Option<crate::sync::SyncHandle>,
  // Stable cross-device id for the current document (sha256 of file bytes).
  pub book_id: Option<String>,
  // Effective sync mode for the current document: the more restrictive of the
  // account-wide server ceiling and this device's local preference. Gates what
  // the reader queues for sync. `Full` (the default) preserves the historical
  // all-in behavior until a document is opted down.
  pub sync_mode: hygg_shared::sync::SyncMode,
  // Account/device-wide automatic-sync scope (from `AUTO_SYNC`): which
  // documents sync without being touched. Combined with `auto_sync_optin` and
  // the book heuristic to decide whether this document auto-syncs.
  pub sync_policy: hygg_shared::sync::AutoSyncPolicy,
  // This device's explicit "auto-sync this document" opt-in, mirrored from the
  // library entry. Lets a non-book document sync under the `books`/`manual`
  // scope; set by `:sync` and `:autosync add`.
  pub auto_sync_optin: bool,
  // Path the current document was opened from (used to derive `book_id`).
  pub source_path: Option<String>,
  // Set by `:home` / `:Rex` to end the reader loop and return to the
  // full-screen home library picker instead of quitting hygg entirely.
  pub exit_to_home: bool,
  // Latest server-side progress for this book awaiting a jump.
  pub pending_server_progress: Option<crate::sync::ServerProgress>,
  // Set when a `pending_server_progress` was deferred because the PDF was
  // still opening (no page table to anchor against yet). Remembers whether
  // it should auto-apply (startup reconcile) or prompt, since the startup
  // window may close before the slow open finishes. Resolved once the stream
  // installs.
  pub pending_server_progress_autoapply: bool,
  // Whether to show the "progress changed on server" status-line prompt.
  pub server_progress_prompt: bool,
  // When the reader first moved while that prompt was up. Starts a short grace
  // after which the prompt clears and the local position overrides the
  // server's (scrolling away = "keep my place"), mirroring the PWA toast.
  pub server_progress_scroll_at: Option<std::time::Instant>,
  // Set when `:server-progress` was invoked without a cached position: it
  // triggers a fresh re-fetch and jumps to whatever the server returns for
  // this book (regardless of newer/older), rather than needing a second
  // invocation. Cleared on the jump or after a short timeout (see
  // `tick_server_progress_grace`).
  pub server_progress_jump_requested_at: Option<std::time::Instant>,
  // Last line we pushed to the server, to suppress prompting on our own echo.
  pub last_synced_offset: Option<usize>,
  // Latest local progress timestamp, including restored progress from disk.
  pub last_local_progress_updated_at: Option<i64>,
  // True until the first successful startup pull completes. During this window
  // newer server progress is applied automatically instead of prompting.
  pub startup_progress_reconcile: bool,
  // Whether the sync server is currently unreachable (set/cleared by the
  // engine's `Connectivity` transitions). Shown as a passive status-line note
  // — the offline analogue of the PWA's connection toast — and cleared the
  // moment a sync cycle succeeds again.
  pub sync_offline: bool,
  // Cumulative active reading time for the current document on this device, in
  // seconds. Seeded from saved progress on open so it accrues across sessions.
  pub reading_time_seconds: u64,
  // Sub-second carry between ticks so short polls aren't lost when rounding to
  // whole seconds.
  pub reading_accrued: f64,
  // Value of `reading_time_seconds` at the last persist, so we can derive the
  // delta to add to the per-day bucket and push to the server.
  pub reading_persisted_seconds: u64,
  // Set when whole seconds accrued since the last persist; lets the snapshot
  // write reading time even when the cursor hasn't moved.
  pub reading_dirty: bool,
  // Wall-clock of the last real keypress; reading time only accrues while the
  // user has been active within `READING_IDLE_SECONDS`.
  pub last_activity: Instant,
  // Instant of the previous accrual tick.
  pub reading_last_tick: Instant,
  // Instant of the last reading-time flush to disk (drives periodic persists
  // while purely reading without moving the cursor).
  pub reading_last_flush: Instant,
}
