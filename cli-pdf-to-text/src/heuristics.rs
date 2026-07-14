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

// Everything below `is_code_like_line` feeds only the native-only,
// layout-fidelity `pdf_to_text` path (plaintext-fallback scoring) and the
// native `stream_recovery` pass. The wasm/PWA build reaches `heuristics` solely
// through `sanitize::labels` → `is_code_like_line`, so the rest is gated off
// wasm to avoid dead-code warnings there.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_heading_like_line(trimmed: &str) -> bool {
  let word_count = trimmed.split_whitespace().count();
  (2..=10).contains(&word_count)
    && trimmed.chars().next().is_some_and(|ch| ch.is_ascii_uppercase())
    && !trimmed.ends_with(['.', ',', ';', ':', '!', '?'])
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_intro_line(trimmed: &str) -> bool {
  if !trimmed.ends_with(':') || is_code_like_line(trimmed) {
    return false;
  }
  let words = trimmed.split_whitespace().count();
  (2..=18).contains(&words)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Clone, Copy)]
struct TextStats {
  non_empty_lines: usize,
  code_like_lines: usize,
  heading_like_lines: usize,
  sparse_intro_blocks: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl TextStats {
  fn quality_score(self) -> usize {
    let richness = self.non_empty_lines
      + self.code_like_lines * 5
      + self.heading_like_lines * 2;
    richness.saturating_sub(self.sparse_intro_blocks * 12)
  }
}

#[cfg(not(target_arch = "wasm32"))]
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

/// Decide whether the layout output looks bad enough that running the
/// (expensive) plaintext extraction as a fallback might be worth it.
///
/// We only do the second extraction when the layout pass shows the kind
/// of damage that `should_prefer_plaintext_output` actually flips for:
/// either at least one detected sparse intro -> code block that the
/// plaintext path can outscore by >= 120%, or a near-empty layout that
/// could plausibly be improved by 40+ lines.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn layout_needs_plaintext_fallback(layout_sanitized: &str) -> bool {
  let layout = analyze_text(layout_sanitized);
  layout.sparse_intro_blocks > 0 || layout.non_empty_lines < 20
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
  use super::layout_needs_plaintext_fallback;

  #[test]
  fn skips_fallback_for_healthy_layout() {
    // A long, varied document with no sparse-intro problems should
    // never trigger the expensive plaintext fallback.
    let mut layout = String::new();
    for _ in 0..50 {
      layout.push_str(
        "This is a normal paragraph of body text in a healthy PDF.\n",
      );
    }
    assert!(!layout_needs_plaintext_fallback(&layout));
  }

  #[test]
  fn triggers_fallback_for_near_empty_layout() {
    // If the layout extractor produced almost nothing, plaintext might
    // still recover content.
    let layout = "Title\n\nSubtitle\n";
    assert!(layout_needs_plaintext_fallback(layout));
  }

  #[test]
  fn triggers_fallback_when_sparse_intro_block_detected() {
    // Intro line ending with ':' immediately followed by a heading
    // (with no code in between) is the exact pattern the recovery
    // heuristic targets — plaintext should at least be inspected.
    let layout = concat!(
      "Here is an example .gitignore file:\n",
      "\n",
      "Another Section Heading\n",
      "\n",
      "Some body text follows here.\n",
    );
    assert!(layout_needs_plaintext_fallback(layout));
  }
}
