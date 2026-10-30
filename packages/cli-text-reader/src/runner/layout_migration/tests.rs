use super::*;
use crate::highlights::Highlight;
use hygg_shared::text_decode::decode_text;

const ENGLISH: &str = "The quick brown fox jumps over the lazy dog. Reading \
  a long document in the terminal should feel calm.\n\nEvery paragraph is \
  justified to the width of the window, and nothing is lost.\n";

/// The layouts before and after hygg decoded text input: the old reader
/// justified the raw file text, the new one justifies the decoded text.
fn layouts(raw: &str, col: usize) -> (Vec<String>, Vec<String>) {
  let decoded = decode_text(raw.as_bytes()).text;
  (cli_justify::justify(raw, col), cli_justify::justify(&decoded, col))
}

fn joined(lines: &[String]) -> String {
  lines.join("\n")
}

/// The byte range of `needle` in justified `text`, where the justifier may
/// have widened or wrapped the spaces between its words.
fn highlight(text: &str, needle: &str) -> Highlight {
  let words: Vec<&str> = needle.split(' ').collect();
  for (start, _) in text.match_indices(words[0]) {
    let mut end = start + words[0].len();
    let all = words[1..].iter().all(|word| {
      let gap = text[end..].len() - text[end..].trim_start().len();
      let found = gap > 0 && text[end + gap..].starts_with(word);
      end += gap + word.len();
      found
    });
    if all {
      return Highlight { start, end, created_at: 0 };
    }
  }
  panic!("{needle:?} not in {text:?}");
}

/// The text of a highlight with whitespace runs collapsed.
fn words(text: &str, h: &Highlight) -> String {
  text[h.start..h.end].split_whitespace().collect::<Vec<_>>().join(" ")
}

fn remapped(map: &LayoutMap, highlights: Vec<Highlight>) -> Vec<Highlight> {
  let mut data = HighlightData::new("doc".into());
  data.highlights = highlights;
  remap_highlights(map, &mut data);
  data.highlights
}

#[test]
fn utf8_bom_highlight_lands_on_the_same_text() {
  let raw = format!("\u{FEFF}{ENGLISH}");
  let (old, new) = layouts(&raw, 30);
  // The BOM was the only difference: same wrapping, first line 3 bytes
  // shorter, so every saved offset was 3 bytes too far.
  assert_eq!(old.len(), new.len());
  assert_eq!(old[0].len(), new[0].len() + 3);

  let (old_text, new_text) = (joined(&old), joined(&new));
  let saved = vec![
    // A selection from the very start, which included the U+FEFF.
    Highlight { start: 0, end: "\u{FEFF}The quick".len(), created_at: 0 },
    highlight(&old_text, "brown fox"),
    highlight(&old_text, "justified to the width"),
  ];
  let map = LayoutMap::new(&old, &new).unwrap();
  let moved = remapped(&map, saved);
  let texts: Vec<String> = moved.iter().map(|h| words(&new_text, h)).collect();
  assert_eq!(texts, ["The quick", "brown fox", "justified to the width"]);
}

#[test]
fn crlf_document_state_follows_its_new_paragraphs() {
  let raw = ENGLISH.replace('\n', "\r\n");
  let (old, new) = layouts(&raw, 30);
  // CRLF hid the paragraph break: the old layout ran both paragraphs together.
  assert!(new.len() > old.len());
  let (old_text, new_text) = (joined(&old), joined(&new));
  let map = LayoutMap::new(&old, &new).unwrap();

  let moved = remapped(&map, vec![highlight(&old_text, "nothing is lost.")]);
  assert_eq!(words(&new_text, &moved[0]), "nothing is lost.");

  // A bookmark on "Every" (line, byte column): in the old layout it sat
  // mid-line, now it starts the second paragraph.
  let line = old.iter().position(|l| l.contains("Every")).unwrap();
  let col = old[line].find("Every").unwrap();
  let marks = remap_marks(&map, &HashMap::from([('a', (line, col))]));
  let (mark_line, mark_col) = marks[&'a'];
  assert!(new[mark_line][mark_col..].starts_with("Every"));

  // A note (or progress) line follows the first word of its old line.
  let first_word = old[line].split_whitespace().next().unwrap();
  let new_line = map.line(line);
  assert!(new[new_line].trim_start().starts_with(first_word));

  // Progress with the viewport on that line.
  let progress = Progress {
    document_hash: 1,
    updated_at: 42,
    offset: line,
    total_lines: old.len(),
    percentage: 50.0,
    viewport_offset: Some(line.saturating_sub(1)),
    cursor_y: Some(line.min(1)),
    page: None,
    line_in_page: None,
    word_offset: Some(crate::word_anchor::words_in_range(&old, &[], 0, line)),
    reading_time_seconds: 7,
  };
  let p = remap_progress(&map, &progress);
  assert_eq!(p.offset, new_line);
  assert_eq!(p.viewport_offset.unwrap() + p.cursor_y.unwrap(), new_line);
  assert_eq!(p.total_lines, new.len());
  assert_eq!(
    p.word_offset,
    Some(crate::word_anchor::words_in_range(&new, &[], 0, new_line))
  );
  assert_eq!((p.updated_at, p.reading_time_seconds), (42, 7));
}

#[test]
fn identity_hash_is_the_one_the_editor_computes() {
  // The editor hashes `raw_content: String`; the migration hashes `&str`.
  let raw = format!("\u{FEFF}{ENGLISH}");
  assert_eq!(generate_hash(&raw.as_str()), generate_hash(&raw));
}

#[test]
fn unchanged_layouts_are_left_alone() {
  assert!(!layout_may_have_changed(ENGLISH));
  assert!(layout_may_have_changed("\u{FEFF}x"));
  assert!(layout_may_have_changed("a\r\nb"));
  // Different characters (e.g. BOM-less UTF-16 that used to open as NUL-laden
  // "UTF-8"): no safe mapping, so nothing is rewritten.
  let old = vec!["H\0i\0".to_string()];
  let new = vec!["Hi".to_string()];
  assert!(LayoutMap::new(&old, &new).is_some()); // NULs are sanitized away
  let new = vec!["Ho".to_string()];
  assert!(LayoutMap::new(&old, &new).is_none());
}
