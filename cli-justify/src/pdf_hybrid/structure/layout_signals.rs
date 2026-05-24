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

  // A wrap-tail fragment like `241)` or `(241` is digits plus a single
  // paren. Punct-density would flag it as a code block (one `)` out of
  // four chars is the 0.25 boundary), so the engine inserts code-block
  // padding above and below — splitting `Plate 1 … page` from the `241)`
  // continuation. Pure numeric/single-punct fragments are wrap leftovers,
  // not code.
  if alpha == 0 && punctuation <= 1 {
    return false;
  }

  let alpha_ratio = alpha as f64 / non_space as f64;
  let punct_ratio = punctuation as f64 / non_space as f64;
  punct_ratio >= 0.25 && alpha_ratio <= 0.80
}

/// Recognises lines from `git log --graph` output so the whole graph
/// stays classified as one code block and doesn't get cli-justify's
/// "transition from prose to code" blank-line padding inserted between
/// adjacent graph rows. Also stops `parse_list_marker` from treating the
/// leading `*` as a bullet-list marker.
///
/// Matches three shapes:
///   * pure-graph rows like `|`, `|\`, `|/`, `* |` — the column-glyph
///     lines that draw the branch topology between commits.
///   * commit rows like `* 2d3acf9 Subject`, `| * 420eac9 Subject`,
///     `* | 30e367c Subject` — where the prefix is graph glyphs and the
///     next token is a 7+ char hex git short-hash.
///   * the same two but indented (we strip leading whitespace before
///     inspecting).
///
/// Conservative on purpose — requires the hash for non-pure-graph rows
/// and a short length for pure-graph rows, so legitimate bullet lists
/// like "* First item in prose" don't get reclassified as code.
pub(crate) fn looks_like_git_log_graph_line(trimmed: &str) -> bool {
  if trimmed.is_empty() {
    return false;
  }
  let mut graph_prefix_chars = 0usize;
  let mut has_non_space_in_prefix = false;
  for ch in trimmed.chars() {
    match ch {
      '*' | '|' | '/' | '\\' => {
        has_non_space_in_prefix = true;
        graph_prefix_chars += 1;
      }
      ' ' => graph_prefix_chars += 1,
      _ => break,
    }
  }
  if !has_non_space_in_prefix {
    return false;
  }

  let rest = &trimmed[graph_prefix_chars..];
  if rest.is_empty() {
    // Pure-graph row (|, |\, |/, * |, etc.). Cap the length so we don't
    // accept long lines that happen to contain only graph characters.
    return trimmed.len() <= 6;
  }

  // Mixed graph + content row: require a git short-hash as the first
  // non-graph token so we don't reclassify ordinary bullet lists.
  let next_token: String =
    rest.chars().take_while(|c| !c.is_whitespace()).collect();
  next_token.len() >= 7
    && next_token.len() <= 40
    && next_token.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn looks_like_code_block_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  // PDF Reference literal-string examples ("( This is a string )", or
  // multi-line `( These \` … `are the same . )`) render with a leading
  // `( ` and read as prose, not code. Without this exception the high
  // punctuation density of opener lines like `( These \` and bare
  // closers like `)` makes them code blocks, which inserts blank-line
  // padding above/below and splits the multi-line example apart.
  if looks_like_pdf_literal_string_line(trimmed) {
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

  if looks_like_git_log_graph_line(trimmed) {
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

fn looks_like_pdf_literal_string_line(trimmed: &str) -> bool {
  // Match PDF Reference literal-string multi-line CONTINUATION patterns
  // only, where the existing code-block padding inserts blank lines in
  // the middle of one logical example:
  //   * `( ... \` — opener with backslash continuation
  //   * bare `)` — standalone closer of a multi-line example
  //   * bare `(` — rare standalone opener
  // Single-line complete examples like `( This is a string )` and
  // `( \0053 )` stay classified as code blocks: there the surrounding
  // blank-line padding helpfully isolates each example from the prose
  // around it (otherwise the example collapses inline into a
  // mid-sentence paragraph join).
  if trimmed == "(" || trimmed == ")" {
    return true;
  }
  trimmed.starts_with("( ") && trimmed.ends_with('\\')
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

#[cfg(test)]
mod tests {
  use super::looks_like_git_log_graph_line;

  #[test]
  fn recognises_pure_graph_rows() {
    assert!(looks_like_git_log_graph_line("|/"));
    assert!(looks_like_git_log_graph_line("|\\"));
    assert!(looks_like_git_log_graph_line("* |"));
    assert!(looks_like_git_log_graph_line("|"));
  }

  #[test]
  fn recognises_commit_rows() {
    assert!(looks_like_git_log_graph_line(
      "* 2d3acf9 Ignore errors from SIGCHLD on trap"
    ));
    assert!(looks_like_git_log_graph_line(
      "* | 30e367c Timeout code and tests"
    ));
    assert!(looks_like_git_log_graph_line(
      "| * 420eac9 Add method for getting the current branch"
    ));
  }

  #[test]
  fn rejects_ordinary_bullet_list_items() {
    // The conservative shape — first non-graph token must be a 7+ char
    // hex string — prevents prose bullet lists from being reclassified.
    assert!(!looks_like_git_log_graph_line("* First item in a prose list"));
    assert!(!looks_like_git_log_graph_line("* Item with multiple words here"));
  }

  #[test]
  fn rejects_unrelated_short_lines() {
    assert!(!looks_like_git_log_graph_line(""));
    assert!(!looks_like_git_log_graph_line("Hello world"));
    assert!(!looks_like_git_log_graph_line("README.md"));
  }
}
