// Kokoro narration worker (feature = "tts").
//
// Runs on a background thread: ensure the model is present, load the engine,
// then chunk the words, synthesize one chunk ahead (synthesis is faster than
// realtime, so the rodio sink stays fed and audio is gapless), play the audio,
// and emit the shared `SpeechMsg::Word` events on the playback clock so the
// existing drain/highlight/auto-scroll path lights up the spoken word.

mod chunking;
mod worker;

#[cfg(test)]
mod tests;

pub(super) use worker::spawn_kokoro_narration;

use crate::editor::speech::WordSpan;
use crate::editor::speech::kokoro::WordAlignment;

// Shared word/chunk type aliases used across the player submodules.
pub(crate) type Word = (WordSpan, String);
pub(crate) type Chunk = (Vec<f32>, Vec<WordAlignment>);
