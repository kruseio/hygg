//! Convert a single raw-ANSI art row (truecolor `▀` half-blocks emitted by
//! `cli-image-to-ascii`) into an HTML span string for `inner_html`.
//!
//! Supports the SGR codes the renderer actually produces: truecolor fg
//! (`38;2;r;g;b`), truecolor bg (`48;2;r;g;b`), and resets (`0` / `39` / `49`).

#[derive(Clone, Copy, Default, PartialEq)]
struct Style {
  fg: Option<(u8, u8, u8)>,
  bg: Option<(u8, u8, u8)>,
}

/// Render an ANSI line to `<span style=…>…</span>` HTML. The caller drops this
/// into a `white-space: pre` element so columns line up.
pub fn ansi_to_html(line: &str) -> String {
  let mut out = String::new();
  let mut style = Style::default();
  let mut pending = String::new();
  let bytes = line.as_bytes();
  let mut i = 0;

  while i < bytes.len() {
    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
      // Flush text accumulated under the current style before it changes.
      flush(&mut out, &mut pending, style);
      let mut j = i + 2;
      while j < bytes.len() && bytes[j] != b'm' {
        j += 1;
      }
      let params = std::str::from_utf8(&bytes[i + 2..j]).unwrap_or("");
      apply(&mut style, params);
      i = if j < bytes.len() { j + 1 } else { j };
    } else {
      // Advance one full UTF-8 char (art uses multibyte `▀`).
      let ch_len = utf8_len(bytes[i]);
      let end = (i + ch_len).min(bytes.len());
      pending.push_str(std::str::from_utf8(&bytes[i..end]).unwrap_or(""));
      i = end;
    }
  }
  flush(&mut out, &mut pending, style);
  out
}

fn flush(out: &mut String, pending: &mut String, style: Style) {
  if pending.is_empty() {
    return;
  }
  let escaped = escape(pending);
  match (style.fg, style.bg) {
    (None, None) => out.push_str(&escaped),
    _ => {
      out.push_str("<span style=\"");
      if let Some((r, g, b)) = style.fg {
        out.push_str(&format!("color:rgb({r},{g},{b});"));
      }
      if let Some((r, g, b)) = style.bg {
        out.push_str(&format!("background:rgb({r},{g},{b});"));
      }
      out.push_str("\">");
      out.push_str(&escaped);
      out.push_str("</span>");
    }
  }
  pending.clear();
}

fn apply(style: &mut Style, params: &str) {
  let toks: Vec<u16> =
    params.split(';').filter_map(|t| t.parse().ok()).collect();
  if toks.is_empty() {
    *style = Style::default();
    return;
  }
  let mut k = 0;
  while k < toks.len() {
    match toks[k] {
      0 => *style = Style::default(),
      39 => style.fg = None,
      49 => style.bg = None,
      38 | 48 if toks.get(k + 1) == Some(&2) && k + 4 < toks.len() => {
        let rgb = (toks[k + 2] as u8, toks[k + 3] as u8, toks[k + 4] as u8);
        if toks[k] == 38 {
          style.fg = Some(rgb);
        } else {
          style.bg = Some(rgb);
        }
        k += 4;
      }
      _ => {}
    }
    k += 1;
  }
}

fn escape(s: &str) -> String {
  s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn utf8_len(first: u8) -> usize {
  match first {
    b if b < 0x80 => 1,
    b if b >> 5 == 0b110 => 2,
    b if b >> 4 == 0b1110 => 3,
    b if b >> 3 == 0b11110 => 4,
    _ => 1,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn plain_text_passes_through_escaped() {
    assert_eq!(ansi_to_html("a < b & c"), "a &lt; b &amp; c");
  }

  #[test]
  fn truecolor_fg_bg_becomes_span() {
    let html =
      ansi_to_html("\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m\u{2580}\x1b[0m");
    assert!(html.contains("color:rgb(255,0,0)"));
    assert!(html.contains("background:rgb(0,0,255)"));
    assert!(html.contains('\u{2580}'));
  }
}
