use crate::text_utils::{char_len, split_at_char};

pub(crate) fn justify_line(line: &[&str], line_width: usize) -> String {
  let word_len: usize = line.iter().map(|s| char_len(s)).sum();

  if word_len >= line_width || line.len() <= 1 {
    return line.join(" ");
  }

  let spaces = line_width - word_len;
  let line_len_div = if line.len() > 1 { line.len() - 1 } else { 1 };
  let each_space = spaces / line_len_div;
  let extra_space = spaces % line_len_div;

  let mut justified = String::new();
  for (i, word) in line.iter().enumerate() {
    justified.push_str(word);
    if i < line.len() - 1 {
      let mut space = " ".repeat(each_space);
      if i < extra_space {
        space.push(' ');
      }
      justified.push_str(&space);
    }
  }

  justified
}

pub fn justify(text: &str, line_width: usize) -> Vec<String> {
  let paragraphs: Vec<&str> = text.split("\n\n").collect();
  let mut lines: Vec<String> = Vec::new();

  for paragraph in paragraphs {
    let raw_words: Vec<&str> = paragraph.split_whitespace().collect();
    let mut words = vec![];

    for mut word in raw_words {
      while let (w1, Some(w2)) = split_at_char(word, line_width) {
        words.push(w1);
        word = w2;
      }

      words.push(word);
    }

    let mut line: Vec<&str> = Vec::new();
    let mut len = 0;

    for word in words {
      let word_len = char_len(word);
      let space_len = if line.is_empty() { 0 } else { 1 };
      let new_len = len + space_len + word_len;

      if new_len > line_width && !line.is_empty() {
        lines.push(justify_line(&line, line_width));
        line.clear();
        len = 0;
      }

      line.push(word);
      len = if line.len() == 1 { word_len } else { len + space_len + word_len };
    }

    if !line.is_empty() {
      lines.push(line.join(" "));
    }

    lines.push(String::new());
  }

  lines
}

#[cfg(test)]
mod tests {
  use super::justify;
  use crate::text_utils::char_len;

  #[test]
  fn handles_long_words() {
    let input_text = r#"some text and a very loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong word but no cause to panic"#;
    let result = justify(input_text, 10);
    assert!(!result.is_empty());
  }

  #[test]
  fn handles_line_longer_than_width() {
    let input_text =
      "This is a line that is definitely longer than the requested width";
    let result = justify(input_text, 20);
    assert!(!result.is_empty());
  }

  #[test]
  fn splits_single_word_longer_than_width() {
    let input_text = "supercalifragilisticexpialidocious";
    let result = justify(input_text, 10);
    assert!(!result.is_empty());
    assert!(result.len() > 1);
  }

  #[test]
  fn normal_justification_produces_full_lines() {
    let input_text = "This is a test of the justification system. It should properly justify lines that need to be wrapped.";
    let result = justify(input_text, 20);
    assert!(!result.is_empty());

    let mut found_justified = false;
    for (i, line) in result.iter().enumerate() {
      if !line.is_empty() && i < result.len() - 2 && char_len(line) == 20 {
        found_justified = true;
        break;
      }
    }
    assert!(found_justified, "Should have at least one justified line");
  }

  #[test]
  fn keeps_unicode_justification_width_stable() {
    let input = "Chapter “Text” introduces Unicode-aware width handling.";
    let result = justify(input, 24);
    assert!(
      result
        .iter()
        .filter(|line| !line.is_empty())
        .take(result.len().saturating_sub(2))
        .all(|line| char_len(line) <= 24),
      "expected all non-final wrapped lines to fit char width, got: {result:?}"
    );
  }
}
