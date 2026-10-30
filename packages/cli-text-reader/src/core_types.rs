// Core type definitions for the editor
use cli_pdf_to_text::PdfLineKind;

use crate::editor::char_boundary::{CursorCol, DisplayCol, LineSpan, Pos};

// Split position for split buffers
#[derive(Clone, Debug, PartialEq)]
pub enum SplitPosition {
  Top,
  Bottom,
}

// View mode for normal or overlay view
#[derive(Clone, Debug, PartialEq)]
pub enum ViewMode {
  Normal,
  Overlay,         // Full-screen overlay buffer
  HorizontalSplit, // Horizontal split view with command output
}

// How a reader session ended. Returned by `Editor::run` so the launcher can
// either exit hygg (`Quit`) or bring the user back to the full-screen home
// library picker (`Home`, triggered by the `:home` / `:Rex` commands) — the
// same view shown when hygg is launched with no input file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunOutcome {
  Quit,
  Home,
}

// Editor modes
#[derive(PartialEq, Debug, Clone)]
#[allow(dead_code)]
pub enum EditorMode {
  Normal,
  Command,
  CommandExecution,
  Search,
  ReverseSearch,
  VisualChar,
  VisualLine,
  Tutorial, // Interactive tutorial mode
}

// Buffer state for managing multiple buffers
#[derive(Clone)]
pub struct BufferState {
  pub lines: Vec<String>,
  pub line_kinds: Vec<PdfLineKind>,
  pub offset: usize,
  pub cursor_x: CursorCol,
  pub cursor_y: usize,
  pub search_query: String,
  pub current_match: Option<LineSpan>,
  pub selection_start: Option<Pos>,
  pub selection_end: Option<Pos>,
  pub viewport_height: usize,
  pub viewport_start: usize,
  pub command: Option<String>,
  // Per-buffer editor state
  pub mode: EditorMode,
  pub command_buffer: String,
  pub command_cursor_pos: usize,
  pub overlay_level: usize, // 0 for main buffer, 1+ for overlays
  // Split-specific properties
  pub split_height: Option<usize>,
  pub is_split_buffer: bool,
  pub split_position: SplitPosition,
}

impl BufferState {
  pub fn new(lines: Vec<String>) -> Self {
    let line_kinds = vec![PdfLineKind::Text; lines.len()];
    Self {
      lines,
      line_kinds,
      offset: 0,
      cursor_x: CursorCol::ZERO,
      cursor_y: 0,
      search_query: String::new(),
      current_match: None,
      selection_start: None,
      selection_end: None,
      viewport_height: 0,
      viewport_start: 0,
      command: None,
      mode: EditorMode::Normal,
      command_buffer: String::new(),
      command_cursor_pos: 0,
      overlay_level: 0,
      split_height: None,
      is_split_buffer: false,
      split_position: SplitPosition::Bottom,
    }
  }
}

// Editor state - tracks shared state across buffers
pub struct EditorState {
  // Shared state across all buffers
  pub yank_buffer: String,
  pub operator_pending: Option<char>, // For vim operations like 'y'
  pub search_direction: bool,
  // Temporary fields for migration - will be removed
  pub mode: EditorMode, // DEPRECATED: Use active buffer's mode
  pub command_buffer: String, // DEPRECATED: Use active buffer's command_buffer
  pub command_cursor_pos: usize, /* DEPRECATED: Use active buffer's
                         * command_cursor_pos */
  pub search_query: String, // DEPRECATED: Use active buffer's search_query
  #[allow(dead_code)]
  pub last_search_index: Option<usize>,
  pub current_match: Option<LineSpan>, /* DEPRECATED: Use
                                        * active buffer's
                                        * current_match */
  pub selection_start: Option<Pos>, /* DEPRECATED: Use active buffer's
                                     * selection_start */
  pub selection_end: Option<Pos>, /* DEPRECATED: Use active
                                   * buffer's selection_end */
  pub visual_selection_active: bool, /* Track if visual selection should be
                                      * preserved */
  pub previous_visual_mode: Option<EditorMode>, /* Track which visual mode
                                                 * we came from */
  // Last visual selection for gv command
  pub last_visual_start: Option<Pos>,
  pub last_visual_end: Option<Pos>,
  pub last_visual_mode: Option<EditorMode>, // VisualChar or VisualLine
  // Search preview state for vim-like behavior
  pub search_preview_active: bool,
  // (cursor_y, cursor_x) to restore when a search is cancelled; `cursor_y` is
  // a viewport row, not a document line.
  pub search_original_cursor: Option<(usize, CursorCol)>,
  pub search_original_offset: Option<usize>,
  pub search_preview_match: Option<LineSpan>, // for highlighting
  pub command_completion: Option<String>,
  // vim's curswant: the on-screen column j/k aim for. See `DesiredColumn`.
  pub desired_column: Option<DesiredColumn>,
}

// The display column j/k try to keep, so a pass over a short line (or a line
// of wide CJK) does not drag the cursor left for good. `line` and `cursor_x`
// record where the cursor was left when the column was last used; if the
// cursor is anywhere else by the next j/k, some other motion moved it and the
// column is measured afresh from the cursor instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesiredColumn {
  // Display column, or `DisplayCol::END` after `$` for "end of every line".
  pub column: DisplayCol,
  pub line: usize,
  pub cursor_x: CursorCol,
}

impl EditorState {
  pub fn new() -> Self {
    Self {
      yank_buffer: String::new(),
      operator_pending: None,
      search_direction: true,
      // Temporary fields for migration
      mode: EditorMode::Normal,
      command_buffer: String::new(),
      command_cursor_pos: 0,
      search_query: String::new(),
      last_search_index: None,
      current_match: None,
      selection_start: None,
      selection_end: None,
      visual_selection_active: false,
      previous_visual_mode: None,
      last_visual_start: None,
      last_visual_end: None,
      last_visual_mode: None,
      search_preview_active: false,
      search_original_cursor: None,
      search_original_offset: None,
      search_preview_match: None,
      command_completion: None,
      desired_column: None,
    }
  }
}
