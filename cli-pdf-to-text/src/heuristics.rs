pub(crate) fn is_code_like_line(trimmed: &str) -> bool {
  if trimmed.is_empty() {
    return false;
  }

  let lower = trimmed.to_ascii_lowercase();
  if lower.starts_with("diff ")
    || lower.starts_with("index ")
    || trimmed.starts_with(['#', '$'])
    || trimmed.starts_with("---")
    || trimmed.starts_with("+++")
    || trimmed.starts_with("@@")
  {
    return true;
  }

  let word_count = trimmed.split_whitespace().count();
  word_count <= 2
    && (trimmed.contains('*')
      || trimmed.contains('/')
      || trimmed.contains('\\')
      || trimmed.starts_with('!')
      || trimmed.starts_with('.'))
}

pub(crate) fn is_heading_like_line(trimmed: &str) -> bool {
  let word_count = trimmed.split_whitespace().count();
  (2..=10).contains(&word_count)
    && trimmed.chars().next().is_some_and(|ch| ch.is_ascii_uppercase())
    && !trimmed.ends_with(['.', ',', ';', ':', '!', '?'])
}

pub(crate) fn is_intro_line(trimmed: &str) -> bool {
  if !trimmed.ends_with(':') || is_code_like_line(trimmed) {
    return false;
  }
  let words = trimmed.split_whitespace().count();
  (2..=18).contains(&words)
}

#[derive(Default, Clone, Copy)]
struct TextStats {
  non_empty_lines: usize,
  code_like_lines: usize,
  heading_like_lines: usize,
  sparse_intro_blocks: usize,
}

impl TextStats {
  fn quality_score(self) -> usize {
    let richness = self.non_empty_lines
      + self.code_like_lines * 5
      + self.heading_like_lines * 2;
    richness.saturating_sub(self.sparse_intro_blocks * 12)
  }
}

fn analyze_text(text: &str) -> TextStats {
  let lines: Vec<&str> = text.lines().collect();
  let mut stats = TextStats::default();

  for (idx, line) in lines.iter().enumerate() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    stats.non_empty_lines += 1;
    if is_code_like_line(trimmed) {
      stats.code_like_lines += 1;
    }
    if is_heading_like_line(trimmed) {
      stats.heading_like_lines += 1;
    }

    if !is_intro_line(trimmed) {
      continue;
    }

    let mut following_code_like = 0usize;
    let mut saw_heading_after_intro = false;
    for next in lines.iter().skip(idx + 1) {
      let next_trimmed = next.trim();
      if next_trimmed.is_empty() {
        continue;
      }
      if is_heading_like_line(next_trimmed) {
        saw_heading_after_intro = true;
        break;
      }
      if is_code_like_line(next_trimmed) {
        following_code_like += 1;
      }
      if following_code_like > 1 {
        break;
      }
    }

    if saw_heading_after_intro && following_code_like <= 1 {
      stats.sparse_intro_blocks += 1;
    }
  }

  stats
}

pub(crate) fn should_prefer_plaintext_output(
  layout_sanitized: &str,
  plaintext_sanitized: &str,
) -> bool {
  let layout = analyze_text(layout_sanitized);
  let plaintext = analyze_text(plaintext_sanitized);

  if plaintext.non_empty_lines <= layout.non_empty_lines + 40 {
    return false;
  }

  plaintext.quality_score().saturating_mul(100)
    >= layout.quality_score().saturating_mul(120)
}
