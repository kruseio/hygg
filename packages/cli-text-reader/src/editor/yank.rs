use super::char_boundary::{ByteCol, Pos, grapheme_base_chars, slice};
use super::core::Editor;

// Emit OSC 52 so the outermost terminal (e.g. Alacritty on a local machine
// driving an SSH session) also receives the copied text. arboard only writes
// to the local NSPasteboard/X11/Wayland clipboard; OSC 52 rides the TTY
// stream and is honored by whichever terminal emulator sits at the end of
// the chain. Combined, both clipboards get the text.
fn osc52_copy(text: &str) {
  use std::io::{IsTerminal, Write};
  let mut stdout = std::io::stdout();
  // Off means a yank stays on this machine — the arboard write above still
  // populates the local clipboard, this only decides whether the sequence that
  // reaches through to the outermost terminal's clipboard is emitted. On by
  // default, so nothing changes for anyone who has not set ENABLE_OSC52=false.
  if !stdout.is_terminal() || !crate::config::osc52_enabled_setting() {
    return;
  }
  let encoded = base64_encode(text.as_bytes());
  let seq = format!("\x1b]52;c;{}\x07", encoded);
  let _ = stdout.write_all(seq.as_bytes());
  let _ = stdout.flush();
}

fn base64_encode(input: &[u8]) -> String {
  const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
  let (chunks, rem) = input.as_chunks::<3>();
  for chunk in chunks {
    let n =
      ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
    out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
    out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
    out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
    out.push(ALPHABET[(n & 0x3F) as usize] as char);
  }
  match rem.len() {
    1 => {
      let n = (rem[0] as u32) << 16;
      out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
      out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
      out.push('=');
      out.push('=');
    }
    2 => {
      let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
      out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
      out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
      out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
      out.push('=');
    }
    _ => {}
  }
  out
}

impl Editor {
  // Yank selected text to buffer and system clipboard
  pub fn yank_selection(&mut self) {
    let selected_text = self.get_selected_text();
    if !selected_text.is_empty() {
      self.editor_state.yank_buffer = selected_text.clone();

      // Copy to system clipboard if available
      if let Some(clipboard) = &mut self.clipboard {
        let _ = clipboard.set_text(&selected_text);
      }
      osc52_copy(&selected_text);

      // Track yank for tutorial
      if self.tutorial_active {
        self.tutorial_yank_performed = true;
      }
    }
  }

  // Yank current line
  pub fn yank_line(&mut self) {
    let cursor_line = self.offset + self.cursor_y;
    self.debug_log_event(
      "yank",
      "yank_line_start",
      &format!("cursor_line={}, total_lines={}", cursor_line, self.lines.len()),
    );

    if cursor_line < self.lines.len() && !self.is_ansi_art_line(cursor_line) {
      let line_text = self.lines[cursor_line].clone();
      self.editor_state.yank_buffer = line_text.clone();
      self.debug_log_state("yank", "yanked_line", &line_text);
      self.debug_log_state(
        "yank",
        "yank_buffer_updated",
        &self.editor_state.yank_buffer,
      );

      // Copy to system clipboard if available
      if let Some(clipboard) = &mut self.clipboard {
        match clipboard.set_text(&self.editor_state.yank_buffer) {
          Ok(_) => self.debug_log_event(
            "yank",
            "clipboard_success",
            "copied to system clipboard",
          ),
          Err(e) => self.debug_log_error(&format!("clipboard_failed: {e}")),
        }
      } else {
        self.debug_log_event(
          "yank",
          "clipboard_unavailable",
          "no system clipboard",
        );
      }
      osc52_copy(&self.editor_state.yank_buffer);

      // Track yank for tutorial
      if self.tutorial_active {
        self.tutorial_yank_performed = true;
      }
    } else {
      self.debug_log_error(&format!(
        "yank_line_bounds_error: cursor_line={}, total_lines={}",
        cursor_line,
        self.lines.len()
      ));
    }
  }

  // Yank word under cursor
  pub fn yank_word(&mut self) {
    let Pos { line: line_idx, col: col_idx } = self.get_cursor_position();
    if line_idx < self.lines.len() && !self.is_ansi_art_line(line_idx) {
      let line = &self.lines[line_idx];

      // Find word boundaries. Indices into `line` are bytes, so walking with
      // `chars()` and slicing by those counts split the 'é' of "café" and
      // panicked; and a char walk also drops a combining accent that is not
      // alphanumeric itself ("cafe\u{301}"). Walk grapheme clusters, each
      // classified by its first char, and slice the line by the byte offsets
      // of the first and one-past-last graphemes (issue #99).
      let chars = grapheme_base_chars(line);
      let col_idx = col_idx.grapheme_index(line);
      if col_idx < chars.len() {
        let mut start = col_idx;
        while start > 0
          && chars
            .get(start - 1)
            .is_some_and(|c| !c.is_whitespace() && c.is_alphanumeric())
        {
          start -= 1;
        }

        let mut end = col_idx;
        while end < chars.len()
          && chars
            .get(end)
            .is_some_and(|c| !c.is_whitespace() && c.is_alphanumeric())
        {
          end += 1;
        }

        if start < end {
          let start = ByteCol::from_grapheme_index(line, start);
          let end = ByteCol::from_grapheme_index(line, end);
          self.editor_state.yank_buffer = slice(line, start, end).to_string();

          // Copy to system clipboard if available
          if let Some(clipboard) = &mut self.clipboard {
            let _ = clipboard.set_text(&self.editor_state.yank_buffer);
          }
          osc52_copy(&self.editor_state.yank_buffer);

          // Track yank for tutorial
          if self.tutorial_active {
            self.tutorial_yank_performed = true;
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::char_boundary::CursorCol;
  use cli_pdf_to_text::PdfLineKind;

  #[test]
  fn base64_encode_pads_remainders() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode("café".as_bytes()), "Y2Fmw6k=");
  }

  #[test]
  fn yank_line_skips_ansi_art_rows() {
    let mut editor = Editor::new(
      vec!["plain".to_string(), "\x1b[38;2;1;2;3m▀\x1b[0m".to_string()],
      80,
    );
    editor.line_kinds = vec![PdfLineKind::Text, PdfLineKind::AnsiArt];
    editor.editor_state.yank_buffer = "previous".to_string();
    editor.cursor_y = 1;

    editor.yank_line();

    assert_eq!(editor.editor_state.yank_buffer, "previous");
  }

  #[test]
  fn yank_word_on_multibyte_word_does_not_panic() {
    // "café" is five bytes; the fourth byte is inside 'é'. Walking with
    // chars().nth() but slicing by those counts used to slice there and panic.
    let mut editor = Editor::new(vec!["café here".to_string()], 80);
    editor.cursor_y = 0;
    editor.cursor_x = CursorCol::lit(0);

    editor.yank_word();

    assert_eq!(editor.editor_state.yank_buffer, "café");
  }

  #[test]
  fn yank_word_after_multibyte_word_uses_byte_cursor() {
    // "canción" is 8 bytes but 7 chars, so "está" starts at byte 9 (char 8)
    // and "aquí€" at byte 15 (char 13). Indexing the char vector with the byte
    // column used to land two characters into the wrong place.
    let line = "canción está aquí€ fin";
    let mut editor = Editor::new(vec![line.to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;

    editor.cursor_x = CursorCol::lit(line.find("está").unwrap());
    assert_eq!(editor.cursor_x.raw(), 9);
    editor.yank_word();
    assert_eq!(editor.editor_state.yank_buffer, "está");

    // Cursor in the middle of a word, on the 'í' of "aquí".
    editor.cursor_x = CursorCol::lit(line.find('í').unwrap());
    editor.yank_word();
    assert_eq!(editor.editor_state.yank_buffer, "aquí");

    editor.cursor_x = CursorCol::lit(line.find("fin").unwrap());
    editor.yank_word();
    assert_eq!(editor.editor_state.yank_buffer, "fin");
  }

  #[test]
  fn yank_word_keeps_combining_marks_and_clusters() {
    let line = "cafe\u{301} ที่นี่ नमस्ते";
    let mut editor = Editor::new(vec![line.to_string()], 80);
    editor.offset = 0;
    editor.cursor_y = 0;
    for (col, word) in [
      (0, "cafe\u{301}"),
      (4, "cafe\u{301}"),
      (line.find("นี่").unwrap() + 3, "ที่นี่"),
      (line.find("न").unwrap(), "नमस्ते"),
    ] {
      editor.cursor_x = CursorCol::lit(col);
      editor.yank_word();
      assert_eq!(editor.editor_state.yank_buffer, word, "at {col}");
    }
  }
}
