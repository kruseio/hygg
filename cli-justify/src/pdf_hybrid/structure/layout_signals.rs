use crate::text_utils::is_ascii_numeric;

const CODE_MARKERS: [&str; 14] = [
  "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", ":=", "+=", "-=", "/*",
  "*/",
];

const PROMPT_PREFIXES: [&str; 7] = ["$", "#", ">", "%", ">>", ">>>", "PS>"];

fn looks_like_prompt_prefix(token: &str) -> bool {
  if PROMPT_PREFIXES.contains(&token) {
    return true;
  }

  token.len() <= 6
    && token.ends_with('>')
    && token
      .chars()
      .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '>'))
}

pub(crate) fn looks_like_command_prompt_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  let Some(first_token) = trimmed.split_whitespace().next() else {
    return false;
  };
  if !looks_like_prompt_prefix(first_token) {
    return false;
  }

  trimmed
    .get(first_token.len()..)
    .is_some_and(|rest| !rest.trim_start().is_empty())
}

fn looks_like_fragmented_token_line(trimmed: &str) -> bool {
  let Some(first) = trimmed.chars().next() else {
    return false;
  };

  if first == '!' && !trimmed.contains(char::is_whitespace) {
    return true;
  }

  if (first == '-' || first == '+')
    && trimmed.chars().nth(1).is_some_and(|ch| ch != ' ' && ch != first)
  {
    let word_count = trimmed.split_whitespace().count();
    return word_count <= 3;
  }

  if first == '.'
    && trimmed.chars().nth(1).is_some_and(|ch| ch.is_ascii_alphanumeric())
  {
    let word_count = trimmed.split_whitespace().count();
    if word_count > 4 {
      return false;
    }
    let first_token = trimmed.split_whitespace().next().unwrap_or_default();
    if ends_token_like_path(first_token) {
      return true;
    }
    return word_count <= 2;
  }

  false
}

fn ends_token_like_path(token: &str) -> bool {
  token.contains('/') || token.contains('\\') || token.ends_with('/')
}

fn looks_like_flag_cluster_line(trimmed: &str) -> bool {
  if !trimmed.starts_with("--") {
    return false;
  }

  let first_token = trimmed.split_whitespace().next().unwrap_or_default();
  if first_token.ends_with(['.', ',', ';', ':']) {
    return false;
  }

  trimmed.split_whitespace().count() <= 3
}

fn looks_like_numbered_label_caption(trimmed: &str) -> bool {
  let mut words = trimmed.split_whitespace();
  let Some(label) = words.next() else {
    return false;
  };
  let Some(number) = words.next() else {
    return false;
  };

  if !label.chars().all(|ch| ch.is_ascii_uppercase()) || label.len() < 5 {
    return false;
  }
  if !number
    .trim_matches([':', '.', ')'])
    .chars()
    .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '/'))
  {
    return false;
  }

  words.next().is_some()
}

fn symbol_density_looks_like_code(trimmed: &str) -> bool {
  let word_count = trimmed.split_whitespace().count();
  if word_count > 6 {
    return false;
  }

  let mut alpha = 0usize;
  let mut non_space = 0usize;
  let mut punctuation = 0usize;
  for ch in trimmed.chars() {
    if ch.is_whitespace() {
      continue;
    }
    non_space += 1;
    if ch.is_alphabetic() {
      alpha += 1;
    } else if !ch.is_ascii_digit() {
      punctuation += 1;
    }
  }
  if non_space == 0 {
    return false;
  }

  let alpha_ratio = alpha as f64 / non_space as f64;
  let punct_ratio = punctuation as f64 / non_space as f64;
  punct_ratio >= 0.25 && alpha_ratio <= 0.80
}

pub(crate) fn looks_like_code_block_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  if looks_like_command_prompt_line(trimmed) {
    return true;
  }
  if looks_like_numbered_label_caption(trimmed) {
    return false;
  }

  if trimmed.starts_with("---")
    || trimmed.starts_with("+++")
    || trimmed.starts_with("@@")
    || trimmed.starts_with("```")
    || trimmed.starts_with("~~~")
  {
    return true;
  }

  if looks_like_flag_cluster_line(trimmed) {
    return true;
  }

  if looks_like_fragmented_token_line(trimmed) {
    return true;
  }

  let leading_ws =
    line.chars().take_while(|&ch| ch == ' ' || ch == '\t').count();
  let word_count = trimmed.split_whitespace().count();
  if word_count <= 2
    && (trimmed.contains('/')
      || trimmed.contains('\\')
      || trimmed.contains('*'))
  {
    // An isolated path token sitting at the left margin is almost always
    // body-text prose (e.g. "[path]/etc/gitconfig.") rather than a code
    // block line, which would normally be indented to align with the rest
    // of the snippet. Require some indentation OR an obvious code prefix.
    if leading_ws >= 2 || trimmed.ends_with('/') {
      return true;
    }
  }

  if has_strong_code_marker_signal(trimmed) {
    return true;
  }

  symbol_density_looks_like_code(trimmed)
}

fn has_strong_code_marker_signal(trimmed: &str) -> bool {
  let word_count = trimmed.split_whitespace().count();
  let marker_hits: usize =
    CODE_MARKERS.iter().map(|marker| trimmed.matches(marker).count()).sum();
  if marker_hits == 0 {
    return false;
  }
  if word_count <= 4 {
    return true;
  }
  marker_hits >= 2 && word_count <= 8
}

pub(crate) fn looks_like_toc_entry(trimmed: &str) -> bool {
  let dot_count = trimmed.chars().filter(|&ch| ch == '.').count();
  if dot_count < 4 {
    return false;
  }
  if !(trimmed.contains("...") || trimmed.contains(". .")) {
    return false;
  }

  trimmed.split_whitespace().last().is_some_and(is_ascii_numeric)
}
