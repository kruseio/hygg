// Local Kokoro-82M TTS engine (feature = "tts").
//
// A lean port of the inference + word-alignment logic from the Kokoros project
// (Apache-2.0): grapheme->phoneme via espeak-ng (`espeak-rs`), tokenize against
// the Kokoro vocab, run the *timestamped* ONNX model via `ort`, and convert the
// per-token duration output into per-word (word, start_sec, end_sec) timings.
//
// Deliberately excludes Kokoros' opus/mp3/ogg encoders, async runtime, and
// HTTP server: hygg only needs raw f32 PCM @ 24 kHz plus the word timings.
// The model is fetched on first use (it is far too large to bundle/publish).

mod align;
mod common;
mod engine;
mod files;

#[cfg(test)]
mod tests;

// Re-export everything previously reachable as `kokoro::...` from the parent
// `speech` module, preserving the original `pub(super)` visibility (widened to
// `pub(crate)` per the cross-module split).
pub(crate) use align::WordAlignment;
pub(crate) use common::SAMPLE_RATE;
pub(crate) use engine::KokoroEngine;
pub(crate) use files::ensure_models;
