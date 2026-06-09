// Utterance chunking and word↔alignment mapping for the Kokoro narration
// worker. Pure (no threading/audio) so it is unit-testable in isolation.

use crate::editor::speech::WordSpan;
use crate::editor::speech::kokoro::WordAlignment;

use super::Word;

// Narration utterance sizing. Short, sentence-aligned utterances keep Kokoro
// accurate (long inputs make it slur or drop words) and start playing fast; the
// lower bound keeps each chunk's playback long enough to hide the next chunk's
// synthesis (avoiding sink underruns). These are quality/latency knobs —
// `KokoroEngine::synthesize` still splits anything near the model token limit,
// so they are not a correctness boundary.
const MIN_CHUNK_WORDS: usize = 8;
const MAX_CHUNK_WORDS: usize = 36;
const MAX_FAST_CHUNK_WORDS: usize = 54;

// Group consecutive on-screen words into narration utterances. Prefer to break
// after sentence-ending punctuation (natural prosody and the most reliable unit
// for the model), but never below MIN_CHUNK_WORDS (so a chunk's audio is long
// enough to cover the next chunk's synthesis) nor above MAX_CHUNK_WORDS (so the
// model stays accurate and well under its token limit). The cap is
// intentionally large enough for common 25-35 word book sentences; splitting
// those mid-sentence makes Kokoro add an audible phrase break even when
// playback is gapless.
pub(crate) fn build_utterance_chunks(
  words: Vec<Word>,
  speed: f32,
) -> Vec<Vec<Word>> {
  let speed = speed.clamp(1.0, 2.0);
  let min_chunk_words = ((MIN_CHUNK_WORDS as f32) * speed).round() as usize;
  let max_chunk_words = (((MAX_CHUNK_WORDS as f32) * speed).round() as usize)
    .min(MAX_FAST_CHUNK_WORDS);

  let mut chunks: Vec<Vec<Word>> = Vec::new();
  let mut cur: Vec<Word> = Vec::new();
  for word in words {
    let ends_sentence = ends_sentence(&word.1);
    cur.push(word);
    if (cur.len() >= min_chunk_words && ends_sentence)
      || cur.len() >= max_chunk_words
    {
      chunks.push(std::mem::take(&mut cur));
    }
  }
  if !cur.is_empty() {
    chunks.push(cur);
  }
  chunks
}

// Does this on-screen word end a sentence? Looks past trailing quotes/brackets
// so `world."` and `(done.)` still count.
fn ends_sentence(word: &str) -> bool {
  word
    .trim_end_matches(['"', '\'', ')', ']', '»', '”'])
    .ends_with(['.', '!', '?'])
}

// A single punctuation token in the *alignment* stream (filtered out so it
// never claims a highlight slot).
pub(crate) fn is_punct(word: &str) -> bool {
  word.len() == 1 && ".,!?:;".contains(word)
}

// An *on-screen* word that is entirely punctuation (".", "...", "?!", …). Such
// words are not spoken and produce no alignment, so the player skips them to
// keep its positional word↔alignment mapping in sync.
fn is_all_punct(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| ".,!?:;".contains(c))
}

// Pair spoken on-screen words with their audio alignments, in order. On-screen
// words that are pure punctuation are not spoken and have no alignment, so they
// are skipped here — keeping the positional mapping 1:1 with `non_punct` so no
// later word is shifted onto the wrong alignment (or dropped off the end).
// Returns each kept word's span and its clamped start time (seconds, relative
// to the chunk's audio start).
pub(crate) fn map_words_to_alignments<'a>(
  spans: &'a [Word],
  non_punct: &[&WordAlignment],
) -> Vec<(&'a WordSpan, f32)> {
  let mut out = Vec::with_capacity(spans.len());
  let mut ai = 0usize;
  for (span, text) in spans {
    if is_all_punct(text) {
      continue;
    }
    let Some(al) = non_punct.get(ai) else {
      break;
    };
    ai += 1;
    out.push((span, al.start_sec.max(0.0)));
  }
  out
}
