// Punctuation/alignment regression tests (no espeak, run in CI).
//
// These guard the espeak-0.2 regression where clause punctuation was dropped
// from the phoneme stream, so Kokoro lost its `,`/`.` pause tokens and ran
// words together (e.g. "two, we" -> "tˈuːwiː"). `assemble_tokens` takes an
// injected phonemizer, so the run/punct/span logic is checked without espeak.

use crate::editor::speech::vocab::tokenize;

use super::super::align::{
  assemble_tokens, is_punct_mark, split_words_and_punct,
};

// One token per alphanumeric char, so a run's joint count equals the sum of its
// words' counts (exact tiling) — lets us assert spans precisely.
fn fake_phon(s: &str) -> Vec<i64> {
  s.chars().filter(|c| c.is_alphanumeric()).map(|_| 99i64).collect()
}

#[test]
fn split_words_and_punct_separates_trailing_marks() {
  assert_eq!(
    split_words_and_punct("Hello, world!"),
    vec!["Hello", ",", "world", "!"]
  );
}

#[test]
fn is_punct_mark_only_matches_single_clause_marks() {
  // Includes the multi-byte em dash / ellipsis / en dash, which the old
  // byte-length check (`s.len() == 1`) would have missed.
  for m in [",", ".", "!", "?", ":", ";", "—", "…", "-", "–"] {
    assert!(is_punct_mark(m), "{m:?} should be a clause mark");
  }
  for s in ["a", "", ",,", "2", " ", "—a"] {
    assert!(!is_punct_mark(s), "{s:?} should not be a clause mark");
  }
  // A standalone hyphen is a clause mark, but an interior one is not —
  // "well-known" must stay a single word.
  assert_eq!(
    split_words_and_punct("a well-known fact"),
    vec!["a", "well-known", "fact"]
  );
}

#[test]
fn assemble_treats_spaced_hyphen_as_em_dash_pause() {
  // PDFs render an em dash as a spaced ASCII hyphen ("usage - how"). The hyphen
  // is absent from the vocab, so it must map to the em dash's pause token
  // rather than tokenizing to nothing (no pause).
  let items = split_words_and_punct("Git usage - how to");
  assert!(items.contains(&"-".to_string()), "hyphen item: {items:?}");

  let (tokens, wmap) = assemble_tokens(&items, fake_phon);
  let dash = tokenize("—");
  assert_eq!(dash.len(), 1, "em dash must be one vocab token");
  assert!(
    tokens.contains(&dash[0]),
    "spaced hyphen must emit the em-dash pause token: {tokens:?}"
  );

  // The hyphen sits strictly between the two clauses.
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(pos("usage") < pos("-") && pos("-") < pos("how"), "{wmap:?}");
}

#[test]
fn assemble_inserts_pauses_and_keeps_words_separated() {
  let items = split_words_and_punct("In Chapter 2, we kitchen.");
  let (tokens, wmap) = assemble_tokens(&items, fake_phon);

  // Comma (id 3) and period (id 4) appear as their own pause tokens.
  assert!(tokens.contains(&3), "comma pause token missing: {tokens:?}");
  assert!(tokens.contains(&4), "period pause token missing: {tokens:?}");

  // Spans tile the stream exactly — no gap, no overlap — so every word indexes
  // the correct per-token durations.
  let mut cursor = 0usize;
  for (_, start, end) in &wmap {
    assert_eq!(*start, cursor, "span gap/overlap in {wmap:?}");
    cursor = *end;
  }
  assert_eq!(cursor, tokens.len(), "spans must cover every token");

  // The regression: the comma sits strictly between "2" and "we".
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(pos("2") < pos(",") && pos(",") < pos("we"), "{wmap:?}");
}

#[test]
fn assemble_treats_em_dash_and_ellipsis_as_pauses() {
  // "Git usage — how to" lost its pause because the em dash was not a clause
  // mark; espeak drops it, so it must be re-emitted as a Kokoro pause token.
  let items = split_words_and_punct("Git usage — how to…");
  assert!(items.contains(&"—".to_string()), "em dash item: {items:?}");
  assert!(items.contains(&"…".to_string()), "ellipsis item: {items:?}");

  let (tokens, wmap) = assemble_tokens(&items, fake_phon);
  let dash = tokenize("—");
  let dots = tokenize("…");
  assert_eq!(dash.len(), 1, "em dash must be one vocab token");
  assert_eq!(dots.len(), 1, "ellipsis must be one vocab token");
  assert!(tokens.contains(&dash[0]), "em dash pause token missing: {tokens:?}");
  assert!(
    tokens.contains(&dots[0]),
    "ellipsis pause token missing: {tokens:?}"
  );

  // The em dash sits strictly between the two clauses.
  let pos = |w: &str| wmap.iter().position(|(t, _, _)| t == w).unwrap();
  assert!(pos("usage") < pos("—") && pos("—") < pos("how"), "{wmap:?}");
}

#[test]
fn assemble_unpunctuated_text_has_no_pause_tokens() {
  let items = split_words_and_punct("just plain words");
  let (tokens, _) = assemble_tokens(&items, fake_phon);
  assert!(!tokens.contains(&3) && !tokens.contains(&4), "{tokens:?}");
}
