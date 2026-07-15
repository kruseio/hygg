//! Resolving a pending server position into this reader's viewport. Split out
//! from `sync_poll` to keep each file within the repository's per-file line
//! budget; behaviour is unchanged.

use super::super::{Editor, restored_pdf_viewport};

impl Editor {
  /// Jump to the pending server position, centering it like resume-on-open.
  pub(crate) fn jump_to_server_progress(&mut self) {
    let Some(progress) = self.pending_server_progress.take() else {
      return;
    };
    self.server_progress_prompt = false;
    self.server_progress_scroll_at = None;
    self.server_progress_jump_requested_at = None;
    self.pending_server_progress_autoapply = false;
    // A server position supersedes the local resume; drop the pending local
    // target so a late page-load can't override the server jump.
    self.pdf_restore_target = None;
    if self.total_lines == 0 {
      return;
    }
    let content_height = self.height.saturating_sub(1);
    let ts = progress.updated_at;

    // 1) Streaming PDF synced from another CLI: restore by (page,
    //    line_in_page).
    // This lands correctly even while pages are still streaming in (a flat
    // offset would point at the wrong row until every page is loaded), and
    // `drain_pdf_stream` keeps the cursor on the same page as content arrives.
    if let (Some(page), Some(line_in_page)) =
      (progress.page, progress.line_in_page)
    {
      let same_pagination =
        progress.total_lines == 0 || progress.total_lines == self.total_lines;
      // Whether the target page's rendering is settled (it and its neighbours
      // are loaded). A placeholder has its own (loading-message) characters,
      // and an unstitched seam shifts the page's rows, so resolving the word
      // anchor — or scaling by this reader's placeholder-shrunk line count —
      // before then would corrupt the row.
      let page_loaded = self.pdf_streaming.as_ref().is_some_and(|s| {
        s.page_render_settled((page as usize).saturating_sub(1))
      });
      // Prefer the exact, width-independent word anchor: resolve it against the
      // target page's own words. Otherwise the page is still exact, but the
      // per-line offset within it isn't comparable across widths (a full-page
      // figure shifts everything), so scale it into this reader's line space.
      let line_in_page = match progress.word_offset {
        Some(word) if page_loaded && !same_pagination => {
          self.page_local_line_for_word(page, word).unwrap_or(line_in_page)
        }
        _ if same_pagination || !page_loaded => line_in_page,
        _ => (line_in_page as f64 * self.total_lines as f64
          / progress.total_lines as f64)
          .round() as usize,
      };
      let cursor_y = same_pagination.then_some(progress.cursor_y).flatten();
      if let Some(line) = self.pdf_line_for_page_position(page, line_in_page) {
        let (o, c) = restored_pdf_viewport(line, content_height, cursor_y);
        self.commit_server_position(o, c, ts);
        if !page_loaded {
          // The landing above was clamped to the placeholder's height. Arm the
          // restore target with the server's own row so the exact position
          // lands the moment the page streams in.
          self.pdf_restore_target = Some(crate::core_state::PdfRestoreTarget {
            page,
            line_in_page,
            cursor_y,
            word_offset: progress.word_offset,
          });
        }
        return;
      }
    }

    // 2) Streaming PDF without a page anchor (synced from a reader that
    // paginates this document differently, e.g. the PWA for PDFs): map the
    // percentage onto this document's pages. The page count is known and stable
    // while pages stream in, so this lands proportionally — a raw line index
    // from the other line-space would land at the wrong place.
    if let Some(line) = self.pdf_line_for_percent(progress.percentage) {
      let (o, c) =
        restored_pdf_viewport(line, content_height, progress.cursor_y);
      self.commit_server_position(o, c, ts);
      return;
    }

    // 3) Exact saved viewport from another reader of the *same* pagination.
    if let (Some(viewport_offset), Some(cursor_y)) =
      (progress.viewport_offset, progress.cursor_y)
      && progress.total_lines == self.total_lines
    {
      let offset = viewport_offset.min(self.total_lines.saturating_sub(1));
      let mut cursor_y = cursor_y.min(content_height.saturating_sub(1));
      if offset + cursor_y >= self.total_lines {
        cursor_y = self.total_lines.saturating_sub(offset + 1);
      }
      self.commit_server_position(offset, cursor_y, ts);
      return;
    }

    // 4) Non-PDF: resolve the width-independent word anchor to this reader's
    //    own
    // line when present (exact), otherwise fall back to the saved line when the
    // line-spaces agree or the percentage mapped onto this reader's line count.
    let target = match progress.word_offset {
      Some(word) if progress.total_lines != self.total_lines => {
        crate::word_anchor::line_for_word_in_range(
          &self.lines,
          &self.line_kinds,
          0,
          self.lines.len(),
          word,
        )
      }
      _ => self.server_target_line(&progress),
    }
    .min(self.total_lines.saturating_sub(1));
    let center = content_height / 2;
    let (offset, cursor_y) = if target < center {
      (0, target)
    } else if target >= self.total_lines.saturating_sub(center)
      && self.total_lines > content_height
    {
      let offset = self.total_lines - content_height;
      (offset, target - offset)
    } else {
      (target.saturating_sub(center), center)
    };
    self.commit_server_position(offset, cursor_y, ts);
  }
}
