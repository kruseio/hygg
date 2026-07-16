fn is_private_use_or_format_char(ch: char) -> bool {
  matches!(
    ch,
    '\u{E000}'..='\u{F8FF}'
      | '\u{F0000}'..='\u{FFFFD}'
      | '\u{100000}'..='\u{10FFFD}'
      | '\u{FEFF}'
      | '\u{200B}'..='\u{200D}'
      | '\u{2060}'
  )
}

/// Cc — the C0 controls, DEL, and the C1 range.
///
/// A terminal does not print these, it obeys them: ESC opens the CSI/OSC
/// sequences that repaint the screen, retitle the window, or hand the clipboard
/// to whoever asked (OSC 52), and U+009B is an 8-bit CSI all by itself. The
/// glyphs on a page cannot ask for any of that, but the bytes behind them are
/// document data — a hostile /Differences or /ToUnicode map can decode a glyph
/// to U+001B, and extracted text is printed to the reader's terminal. No real
/// character maps here, so dropping the whole class costs nothing.
///
/// TAB survives: it is layout, and the rest of the pipeline treats it as such.
/// A newline never reaches this function — callers split into lines first.
///
/// Twin of the check in stream/text_rows.rs, which normalizes the other
/// extraction path; both are on the way from a PDF to a terminal.
fn is_terminal_control_char(ch: char) -> bool {
  ch.is_control() && ch != '\t'
}

pub(crate) fn normalize_extracted_line(line: &str) -> String {
  let mut normalized = String::with_capacity(line.len());
  for ch in line.chars() {
    if is_private_use_or_format_char(ch) || is_terminal_control_char(ch) {
      continue;
    }
    if ch == '\u{00A0}' {
      normalized.push(' ');
      continue;
    }
    normalized.push(ch);
  }
  normalized
}
