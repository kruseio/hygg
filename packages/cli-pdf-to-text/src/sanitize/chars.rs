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

pub(crate) fn normalize_extracted_line(line: &str) -> String {
  let mut normalized = String::with_capacity(line.len());
  for ch in line.chars() {
    if is_private_use_or_format_char(ch) {
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
