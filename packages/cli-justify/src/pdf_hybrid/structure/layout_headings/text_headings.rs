use crate::text_utils::{is_ascii_numeric, leading_whitespace_width};

use crate::pdf_hybrid::structure::layout_signals::{
  looks_like_command_prompt_line, looks_like_toc_entry,
};

const TOKEN_TRIM_CHARS: [char; 14] =
  ['"', '\'', '(', ')', '[', ']', '{', '}', ',', '.', ';', ':', '!', '?'];

fn trim_heading_token(word: &str) -> &str {
  word.trim_matches(TOKEN_TRIM_CHARS.as_slice())
}

pub(crate) fn is_centered_short_heading(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  if trimmed.chars().next().is_some_and(|ch| ch.is_lowercase()) {
    return false;
  }

  let leading_ws = leading_whitespace_width(line);
  let word_count = trimmed.split_whitespace().count();
  leading_ws >= 16
    && word_count <= 6
    && trimmed.len() <= 40
    && !trimmed.ends_with(['.', ',', ';', ':'])
    && !looks_like_toc_entry(trimmed)
}

fn is_heading_connector_word(word: &str) -> bool {
  matches!(
    word,
    "a"
      | "an"
      | "and"
      | "as"
      | "at"
      | "but"
      | "by"
      | "for"
      | "from"
      | "in"
      | "into"
      | "of"
      | "on"
      | "or"
      | "the"
      | "to"
      | "via"
      | "vs"
      | "vs."
      | "with"
      | "without"
      | "is"
      | "are"
      | "was"
      | "were"
      | "be"
      | "been"
      | "being"
      | "do"
      | "does"
      | "did"
      | "has"
      | "have"
      | "had"
      | "can"
      | "could"
      | "shall"
      | "should"
      | "will"
      | "would"
      | "may"
      | "might"
      | "must"
      | "if"
      | "than"
      | "then"
  )
}

fn is_numbered_label_token(token: &str) -> bool {
  let token = token.trim_matches([':', '.', ')']);
  !token.is_empty()
    && token
      .chars()
      .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '/'))
}

fn looks_like_labeled_caption_line(trimmed: &str) -> bool {
  let mut words = trimmed.split_whitespace();
  let Some(label) = words.next() else {
    return false;
  };
  let Some(number) = words.next() else {
    return false;
  };

  (5..=12).contains(&label.len())
    && label.chars().all(|ch| ch.is_ascii_uppercase())
    && is_numbered_label_token(number)
}

pub(crate) fn looks_like_numbered_label_heading(line: &str) -> bool {
  let trimmed = line.trim();
  !trimmed.is_empty()
    && looks_like_labeled_caption_line(trimmed)
    && trimmed.split_whitespace().nth(2).is_some()
}

fn looks_like_title_case_heading_word(word: &str) -> bool {
  let token = trim_heading_token(word);
  if token.is_empty() {
    return false;
  }

  if is_ascii_numeric(token) {
    return true;
  }

  let lowercase = token.to_ascii_lowercase();
  if is_heading_connector_word(&lowercase) {
    return true;
  }

  if token.len() <= 6
    && token.chars().any(|ch| ch.is_alphabetic())
    && token
      .chars()
      .all(|ch| ch.is_uppercase() || ch.is_ascii_digit() || ch == '&')
  {
    return true;
  }

  let mut chars = token.chars();
  let Some(first) = chars.next() else {
    return false;
  };
  if !first.is_uppercase() {
    return false;
  }

  chars.all(|ch| {
    ch.is_lowercase()
      || ch.is_ascii_digit()
      || matches!(ch, '\'' | '-' | '’' | '/' | '&')
  })
}

pub(crate) fn looks_like_single_word_section_heading(line: &str) -> bool {
  let trimmed = line.trim();
  let leading_ws = leading_whitespace_width(line);
  if leading_ws > 6 {
    return false;
  }

  let mut words = trimmed.split_whitespace();
  let Some(only_word) = words.next() else {
    return false;
  };
  if words.next().is_some() {
    return false;
  }

  let chars: Vec<char> = only_word.chars().collect();
  if !(3..=24).contains(&chars.len()) {
    return false;
  }
  let mut iter = chars.iter();
  let Some(&first) = iter.next() else {
    return false;
  };
  if !first.is_uppercase() {
    return false;
  }
  let mut letters = 1usize;
  for &ch in iter {
    if ch.is_lowercase() || matches!(ch, '-' | '\'' | '’') {
      letters += 1;
    } else {
      return false;
    }
  }
  letters >= 3
}

pub(crate) fn looks_like_left_aligned_section_heading(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  let Some(first) = trimmed.chars().next() else {
    return false;
  };
  if !first.is_uppercase() && !first.is_ascii_digit() {
    return false;
  }

  let leading_ws = leading_whitespace_width(line);
  if leading_ws > 6 {
    return false;
  }

  let word_count = trimmed.split_whitespace().count();
  if !(2..=10).contains(&word_count) {
    return false;
  }

  let char_count = trimmed.chars().count();
  if !(8..=88).contains(&char_count) {
    return false;
  }

  if trimmed.ends_with(['.', ',', ';']) {
    return false;
  }

  if trimmed.contains("://")
    || looks_like_command_prompt_line(trimmed)
    || trimmed.contains("  ")
    || trimmed.contains("   ")
    || looks_like_labeled_caption_line(trimmed)
  {
    return false;
  }

  let mut meaningful_words = 0usize;
  for word in trimmed.split_whitespace() {
    let token = trim_heading_token(word);
    if token.is_empty() {
      return false;
    }
    if token.chars().all(|ch| ch.is_ascii_digit() || ch == '.' || ch == '-') {
      continue;
    }
    if is_heading_connector_word(&token.to_ascii_lowercase()) {
      continue;
    }
    meaningful_words += 1;
    if !looks_like_title_case_heading_word(token) {
      return false;
    }
  }

  meaningful_words >= 2
}
