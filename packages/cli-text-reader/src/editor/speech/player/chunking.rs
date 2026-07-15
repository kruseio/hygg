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

// Two words sit across a paragraph break when a blank line falls between them.
// Lines are hard-wrapped, so adjacent line indices (`line + 1`) are just a
// wrapped continuation of the same paragraph (no pause); a gap of 2+ means a
// blank separator line — a real paragraph/heading boundary.
fn crosses_paragraph(prev: &Word, next: &Word) -> bool {
  next.0.line > prev.0.line + 1
}

// Join a chunk's words into the text handed to the synthesizer, inserting a
// sentence stop at a paragraph break *inside* the chunk when the preceding word
// has no terminal punctuation. A heading or caption with no period that the
// chunker swept up with the following paragraph would otherwise run straight
// into it with no pause; the injected `.` becomes a Kokoro pause token (it is
// filtered back out of the highlight mapping, so word↔alignment stays 1:1).
pub(crate) fn chunk_to_synth_text(chunk: &[Word]) -> String {
  let mut text = String::new();
  for (i, (_, word)) in chunk.iter().enumerate() {
    if i > 0 {
      if crosses_paragraph(&chunk[i - 1], &chunk[i])
        && !ends_sentence(&chunk[i - 1].1)
      {
        text.push('.');
      }
      text.push(' ');
    }
    text.push_str(word);
  }
  text
}

// Silence (seconds) to append after a chunk so the gap to the next chunk reads
// as a real pause. Kokoro renders little trailing silence at a chunk's end (the
// pause normally lives *between* a mark and the next word, which isn't there
// when the chunk stops), and chunks are appended gaplessly — so a sentence or
// paragraph that ends a chunk otherwise runs straight into the next. A
// paragraph break (blank line between) gets a full stop; a sentence end gets a
// shorter one; a chunk split mid-sentence (the phrase keeps flowing) gets none,
// staying gapless.
pub(crate) fn trailing_pause_secs(
  prev_last: &Word,
  next_first: Option<&Word>,
) -> f32 {
  let Some(next_first) = next_first else {
    return 0.0; // last chunk: nothing follows
  };
  if crosses_paragraph(prev_last, next_first) {
    0.50
  } else if ends_sentence(&prev_last.1) {
    0.25
  } else {
    0.0
  }
}

// Turns rodio's per-chunk playback position into a monotonic *global* audio
// position across all queued chunks. `Player::get_pos` reports the position
// within the source currently playing and resets toward zero when playback
// rolls onto the next appended chunk; this folds each finished chunk's duration
// into a running base so callers see one continuous timeline. Pacing word
// highlights against this — instead of a wall clock — keeps them tied to the
// audio actually produced, so an output-device switch that briefly stalls
// playback stalls the highlights with it instead of letting them race ahead.
#[derive(Debug, Default)]
pub(crate) struct AudioClock {
  played_base: f32,   // summed duration of chunks fully played
  playing_idx: usize, // index of the chunk get_pos is reporting
  last_raw: f32,      // previous raw get_pos, to spot the per-chunk reset
}

impl AudioClock {
  pub(crate) fn new() -> Self {
    Self::default()
  }

  // `raw` = `Player::get_pos()` seconds; `chunk_durs` = each appended chunk's
  // duration, in order. Returns the global audio position in seconds. A large
  // backward jump in `raw` means rodio advanced to the next chunk and reset its
  // tracker, so the finished chunk's whole duration rolls into the base. A
  // stall holds `raw` flat (no jump), so the position simply stops advancing.
  // Robust either way: if a backend instead reports a cumulative position, the
  // jump never fires and `raw` is already global.
  pub(crate) fn observe(&mut self, raw: f32, chunk_durs: &[f32]) -> f32 {
    const RESET_DROP: f32 = 0.2;
    if raw + RESET_DROP < self.last_raw
      && self.playing_idx + 1 < chunk_durs.len()
    {
      self.played_base += chunk_durs[self.playing_idx];
      self.playing_idx += 1;
    }
    self.last_raw = raw;
    self.played_base + raw
  }
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
