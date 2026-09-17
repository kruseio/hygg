use crate::text_utils::{display_width, split_at_width};

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

  let first_width = line_width.saturating_sub(display_width(first_prefix));
  let continuation_width =
    line_width.saturating_sub(display_width(continuation_prefix));
  if first_width == 0 || continuation_width == 0 {
    return vec![format!("{first_prefix}{text}")];
  }

  let mut text_lines: Vec<String> = Vec::new();
  let mut current_line = String::new();
  let mut current_width_limit = first_width;

  for mut word in text.split_whitespace() {
    if !current_line.is_empty() {
      let candidate_len =
        display_width(&current_line) + 1 + display_width(word);
      if candidate_len <= current_width_limit {
        current_line.push(' ');
        current_line.push_str(word);
        continue;
      }

      text_lines.push(std::mem::take(&mut current_line));
      current_width_limit = continuation_width;
    }

    while display_width(word) > current_width_limit {
      let (chunk, rest) = split_at_width(word, current_width_limit);
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
    current_line.push_str(word);
  }

  if !current_line.is_empty() {
    text_lines.push(current_line);
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

#[cfg(test)]
mod tests {
  use super::{apply_prefixes, split_last_word, wrap_plain_with_prefix};

  #[test]
  fn prefixes_apply_to_the_first_and_continuation_lines() {
    assert_eq!(
      apply_prefixes(vec!["one".into(), "two".into()], "1  ", "   "),
      ["1  one", "   two"]
    );
  }

  #[test]
  fn empty_titles_and_zero_content_width_keep_the_prefix() {
    assert_eq!(wrap_plain_with_prefix(" \t ", 8, "1  ", "   "), ["1  "]);
    assert_eq!(wrap_plain_with_prefix("中文", 2, "1  ", "   "), ["1  中文"]);
    assert_eq!(wrap_plain_with_prefix("中文", 2, "", "   "), ["中文"]);
  }

  #[test]
  fn oversized_graphemes_and_words_wrap_without_losing_text() {
    assert_eq!(
      wrap_plain_with_prefix("中文中文", 2, "", ""),
      ["中", "文", "中", "文"]
    );
    assert_eq!(wrap_plain_with_prefix("中文", 1, "", ""), ["中", "文"]);
    assert_eq!(
      wrap_plain_with_prefix("one two three", 7, "1  ", "   "),
      ["1  one", "   two", "   thre", "   e"]
    );
  }

  #[test]
  fn final_word_splits_only_when_both_sides_have_text() {
    assert_eq!(split_last_word("one two"), Some(("one".into(), "two".into())));
    assert_eq!(split_last_word("one"), None);
    assert_eq!(split_last_word(" one"), None);
    assert_eq!(split_last_word("one "), None);
  }
}
