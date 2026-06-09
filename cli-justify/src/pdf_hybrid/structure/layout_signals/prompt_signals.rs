use crate::text_utils::is_ascii_numeric;

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

/// Recognises lines from `git log --graph` output so the whole graph
/// stays classified as one code block and doesn't get cli-justify's
/// "transition from prose to code" blank-line padding inserted between
/// adjacent graph rows. Also stops `parse_list_marker` from treating the
/// leading `*` as a bullet-list marker.
///
/// Matches three shapes:
///   * pure-graph rows like `|`, `|\`, `|/`, `* |` — the column-glyph lines
///     that draw the branch topology between commits.
///   * commit rows like `* 2d3acf9 Subject`, `| * 420eac9 Subject`, `* |
///     30e367c Subject` — where the prefix is graph glyphs and the next token
///     is a 7+ char hex git short-hash.
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
