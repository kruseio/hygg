//! Full-text search over the rendered docs: a case-insensitive substring scan
//! of every section, with escaped, `<mark>`-highlighted snippets around the
//! first match.

use super::*;

pub(crate) struct SearchHit {
  pub(crate) page_slug: &'static str,
  pub(crate) page_title: &'static str,
  pub(crate) section_slug: String,
  pub(crate) section_title: String,
  /// Pre-escaped HTML with the query wrapped in `<mark>`.
  pub(crate) snippet: String,
  pub(crate) score: usize,
}

/// Full-text search across every section of every page. Case-insensitive
/// substring match; ranks title matches above body matches and more matches
/// above fewer. Returns at most 60 hits, most relevant first.
pub(crate) fn search_docs(query: &str) -> Vec<SearchHit> {
  let needle = query.trim().to_ascii_lowercase();
  if needle.is_empty() {
    return Vec::new();
  }
  let mut hits: Vec<SearchHit> = Vec::new();
  for doc in docs() {
    for section in &doc.sections {
      let body_matches =
        count_matches(&section.text.to_ascii_lowercase(), &needle);
      let title_match = section.title.to_ascii_lowercase().contains(&needle);
      if body_matches == 0 && !title_match {
        continue;
      }
      let snippet = if body_matches > 0 {
        snippet_html(&section.text, &needle)
      } else {
        lead_html(&section.text)
      };
      hits.push(SearchHit {
        page_slug: doc.slug,
        page_title: doc.title,
        section_slug: section.slug.clone(),
        section_title: section.title.clone(),
        snippet,
        score: body_matches + if title_match { 8 } else { 0 },
      });
    }
  }
  hits.sort_by(|a, b| {
    b.score.cmp(&a.score).then_with(|| a.page_title.cmp(b.page_title))
  });
  hits.truncate(60);
  hits
}

/// Count non-overlapping occurrences of `needle` in `haystack` (both already
/// ASCII-lowercased by the caller).
fn count_matches(haystack: &str, needle: &str) -> usize {
  if needle.is_empty() {
    return 0;
  }
  let mut count = 0;
  let mut i = 0;
  while let Some(pos) = haystack[i..].find(needle) {
    count += 1;
    i += pos + needle.len();
  }
  count
}

/// A ~240-char window around the first match, HTML-escaped with every
/// occurrence of `needle` wrapped in `<mark>`, and `…` where it was clipped.
fn snippet_html(text: &str, needle: &str) -> String {
  let lower = text.to_ascii_lowercase();
  let pos = lower.find(needle).unwrap_or(0);
  let start = char_boundary(text, pos.saturating_sub(80), false);
  let end =
    char_boundary(text, (pos + needle.len() + 160).min(text.len()), true);
  let mut out = String::new();
  if start > 0 {
    out.push('…');
  }
  out.push_str(&highlight(text[start..end].trim(), needle));
  if end < text.len() {
    out.push('…');
  }
  out
}

/// The first ~200 chars of `text`, HTML-escaped (no highlighting): used for
/// index-card descriptions and title-only search hits.
pub(crate) fn lead_html(text: &str) -> String {
  let end = char_boundary(text, 200.min(text.len()), true);
  let mut out = esc(text[..end].trim());
  if end < text.len() {
    out.push('…');
  }
  out
}

/// HTML-escape `text` and wrap each occurrence of `needle` in `<mark>`. Both
/// `text` and `needle` are matched via ASCII-lowercasing, which preserves byte
/// length so the indices line up with the original.
fn highlight(text: &str, needle: &str) -> String {
  if needle.is_empty() {
    return esc(text);
  }
  let lower = text.to_ascii_lowercase();
  let mut out = String::new();
  let mut i = 0;
  while let Some(pos) = lower[i..].find(needle) {
    let start = i + pos;
    let end = start + needle.len();
    out.push_str(&esc(&text[i..start]));
    out.push_str("<mark>");
    out.push_str(&esc(&text[start..end]));
    out.push_str("</mark>");
    i = end;
  }
  out.push_str(&esc(&text[i..]));
  out
}

/// Nudge a byte index to the nearest char boundary (forward or backward) so
/// slicing multi-byte text never panics.
fn char_boundary(text: &str, mut idx: usize, forward: bool) -> usize {
  if idx >= text.len() {
    return text.len();
  }
  while idx > 0 && idx < text.len() && !text.is_char_boundary(idx) {
    if forward {
      idx += 1;
    } else {
      idx -= 1;
    }
  }
  idx
}

pub(crate) fn encode_query(q: &str) -> String {
  url::form_urlencoded::byte_serialize(q.as_bytes()).collect()
}
