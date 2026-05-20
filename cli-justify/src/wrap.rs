use crate::text_utils::{
  char_len, drop_one_leading_whitespace, leading_whitespace, split_at_char,
  split_at_last_whitespace_before,
};

pub(crate) fn wrap_line_preserving_whitespace(
  line: &str,
  line_width: usize,
) -> Vec<String> {
  if line_width == 0 {
    return vec![String::new()];
  }

  if char_len(line) <= line_width {
    return vec![line.to_string()];
  }
  if line.trim_matches([' ', '\t']).is_empty() {
    return vec![String::new()];
  }

  let trimmed_start = line.trim_start_matches([' ', '\t']);
  let original_indent_chars = char_len(line) - char_len(trimmed_start);
  if !trimmed_start.is_empty() && char_len(trimmed_start) <= line_width {
    let clamped_indent = line_width.saturating_sub(char_len(trimmed_start));
    // Only collapse the leading whitespace when the original indent looks
    // truly excessive (e.g. an over-indented TOC label) and clamping is
    // strictly less than the original. Otherwise preserve the indent and
    // let the wrapping loop split the line at word boundaries.
    if original_indent_chars > 20 && clamped_indent < original_indent_chars {
      return vec![format!("{}{}", " ".repeat(clamped_indent), trimmed_start)];
    }
  }

  let indent = leading_whitespace(line).to_string();
  let indent_chars = char_len(&indent);
  let max_continuation_indent =
    line_width.saturating_sub(8).min(32).min(indent_chars);
  let continuation_indent = " ".repeat(max_continuation_indent);
  let continuation_indent_chars = max_continuation_indent;
  let mut out = Vec::new();

  let mut remainder = line;
  let mut is_first = true;

  loop {
    let available = if is_first {
      line_width
    } else {
      line_width.saturating_sub(continuation_indent_chars)
    };

    if available == 0 {
      let (w1, w2) = split_at_char(remainder, 1);
      out.push(if is_first {
        w1.to_string()
      } else {
        format!("{continuation_indent}{w1}")
      });
      remainder = w2.unwrap_or("");
      if remainder.is_empty() {
        break;
      }
      is_first = false;
      continue;
    }

    if char_len(remainder) <= available {
      out.push(if is_first {
        remainder.to_string()
      } else {
        format!("{continuation_indent}{remainder}")
      });
      break;
    }

    let split_byte_idx = split_at_last_whitespace_before(remainder, available)
      .unwrap_or_else(|| {
        let (w1, _) = split_at_char(remainder, available);
        w1.len()
      });

    let (chunk, rest) = remainder.split_at(split_byte_idx);
    let chunk_without_trailing_ws = chunk.trim_end_matches([' ', '\t']);
    if chunk_without_trailing_ws.is_empty() {
      if is_first {
        let trimmed_remainder = remainder.trim_start_matches([' ', '\t']);
        if !trimmed_remainder.is_empty()
          && char_len(trimmed_remainder) <= line_width
        {
          let right_aligned_indent =
            line_width.saturating_sub(char_len(trimmed_remainder));
          out.push(format!(
            "{}{}",
            " ".repeat(right_aligned_indent),
            trimmed_remainder
          ));
          break;
        }
      }

      remainder = rest.trim_start_matches([' ', '\t']);
      is_first = false;
      if remainder.is_empty() {
        out.push(String::new());
        break;
      }
      continue;
    }

    out.push(if is_first {
      chunk_without_trailing_ws.to_string()
    } else {
      format!("{continuation_indent}{chunk_without_trailing_ws}")
    });

    remainder = drop_one_leading_whitespace(rest);
    is_first = false;

    if remainder.is_empty() {
      break;
    }
  }

  out
}

pub fn wrap_preserve_whitespace(text: &str, line_width: usize) -> Vec<String> {
  let mut out = Vec::new();
  for line in text.split('\n') {
    if line.is_empty() {
      out.push(String::new());
      continue;
    }
    out.extend(wrap_line_preserving_whitespace(line, line_width));
  }
  out
}

#[cfg(test)]
mod tests {
  use super::wrap_preserve_whitespace;
  use crate::text_utils::char_len;

  #[test]
  fn preserves_indentation_and_spacing() {
    let input = "a    b    c";
    let out = wrap_preserve_whitespace(input, 80);
    assert_eq!(out, vec![input.to_string()]);

    let input =
      "    fn main() {    println!(\"hi\");    println!(\"there\"); }";
    let out = wrap_preserve_whitespace(input, 25);
    assert!(out.len() > 1);
    assert!(out[0].starts_with("    "));
    assert!(out[1].starts_with("    "));
  }

  #[test]
  fn avoids_vertical_splitting_for_extreme_indentation() {
    let input = format!("{}Contents", " ".repeat(105));
    let out = wrap_preserve_whitespace(&input, 80);
    assert!(out.iter().any(|line| line.contains("Contents")));
    assert!(out.len() <= 3, "expected compact output, got: {out:?}");
    assert!(
      !out.iter().any(|line| line.trim() == "C"),
      "expected no single-letter vertical split, got: {out:?}"
    );
    assert!(
      out
        .iter()
        .filter(|line| !line.is_empty())
        .all(|line| char_len(line) <= 80),
      "expected wrapped lines to respect width, got: {out:?}"
    );
  }

  #[test]
  fn preserves_right_position_for_overindented_short_labels() {
    let input = format!("{}Content", " ".repeat(86));
    let out = wrap_preserve_whitespace(&input, 80);

    assert_eq!(out.len(), 1, "expected single wrapped line, got: {out:?}");
    assert_eq!(
      char_len(&out[0]),
      80,
      "expected width-clamped output, got: {out:?}"
    );
    assert!(
      out[0].ends_with("Content"),
      "expected label text to be preserved, got: {out:?}"
    );
    let leading = out[0].chars().take_while(|&ch| ch == ' ').count();
    assert!(
      leading >= 70,
      "expected right-positioned label instead of collapsed continuation indent, got: {out:?}"
    );
  }
}
