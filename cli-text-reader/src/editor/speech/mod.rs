// TTS narration — Phase 1 vertical slice.
//
// This module proves the *reading UX* end to end with zero ML/audio
// dependencies: it builds per-word spans from the on-screen lines, runs a
// background "fake voice" that emits word-boundary events on a synthetic
// reading clock, and drives a live "spoken word" highlight plus cursor
// auto-scroll through the existing render loop.
//
// The real Kokoro engine (Phase 2, `kokoro` submodule, feature = "tts") emits
// the same `SpeechMsg::Word` events from actual audio timings; everything
// downstream (drain, highlight, auto-scroll) is shared with the fake voice.

#[cfg(feature = "tts")]
mod kokoro;
#[cfg(feature = "tts")]
mod player;
#[cfg(feature = "tts")]
mod vocab;

mod editor_impl;
#[cfg(any(not(feature = "tts"), test))]
mod fake_voice;
mod spans;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use spans::{build_word_spans, word_byte_ranges};
pub(crate) use types::{
  SpeakAction, SpeechMsg, SpeechState, TtsStatus, WordSpan,
};

#[cfg(any(not(feature = "tts"), test))]
pub(crate) use types::{BASE_MS, PER_CHAR_MS, SLEEP_STEP_MS};
