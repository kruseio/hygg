use crate::text_utils::{char_len, split_at_char};

pub(super) fn apply_prefixes(
  lines: Vec<String>,
  first_prefix: &str,
  continuation_prefix: &str,
) -> Vec<String> {
  let mut out = Vec::with_capacity(lines.len().max(1));
  for (idx, line) in lines.into_iter().enumerate() {
    if idx == 0 {
      out.push(format!("{first_prefix}{line}"));
    } else {
      out.push(format!("{continuation_prefix}{line}"));
    }
  }
  out
}

pub(super) fn wrap_plain_with_prefix(
  text: &str,
  line_width: usize,
  first_prefix: &str,
  continuation_prefix: &str,
) -> Vec<String> {
  if text.trim().is_empty() {
    return vec![first_prefix.to_string()];
  }

  let first_width = line_width.saturating_sub(char_len(first_prefix));
  let continuation_width =
    line_width.saturating_sub(char_len(continuation_prefix));
  if first_width == 0 || continuation_width == 0 {
    return vec![format!("{first_prefix}{text}")];
  }

  let mut text_lines: Vec<String> = Vec::new();
  let mut current_line = String::new();
  let mut current_width_limit = first_width;

  for mut word in text.split_whitespace() {
    while char_len(word) > current_width_limit && current_line.is_empty() {
      let (chunk, rest) = split_at_char(word, current_width_limit);
      text_lines.push(chunk.to_string());
      word = rest.unwrap_or("");
      current_width_limit = continuation_width;
      if word.is_empty() {
        break;
      }
    }
    if word.is_empty() {
      continue;
    }

    let word_len = char_len(word);
    if current_line.is_empty() {
      current_line.push_str(word);
      continue;
    }

    let candidate_len = char_len(&current_line) + 1 + word_len;
    if candidate_len <= current_width_limit {
      current_line.push(' ');
      current_line.push_str(word);
      continue;
    }

    text_lines.push(current_line);
    current_line = word.to_string();
    current_width_limit = continuation_width;
  }

  if !current_line.is_empty() {
    text_lines.push(current_line);
  }
  if text_lines.is_empty() {
    text_lines.push(String::new());
  }

  apply_prefixes(text_lines, first_prefix, continuation_prefix)
}

pub(super) fn split_last_word(line: &str) -> Option<(String, String)> {
  let split_idx = line.rfind(' ')?;
  let left = line[..split_idx].trim_end();
  let right = line[split_idx..].trim();
  if left.is_empty() || right.is_empty() {
    return None;
  }
  Some((left.to_string(), right.to_string()))
}
