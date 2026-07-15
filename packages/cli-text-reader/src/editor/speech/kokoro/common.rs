// Shared constants and type aliases for the Kokoro engine submodules.

use std::borrow::Cow;
use std::collections::HashMap;

use ort::session::SessionInputValue;

pub(crate) const SAMPLE_RATE: u32 = 24_000;
pub(crate) const STYLE_DIM: usize = 256;
pub(crate) const MAX_STYLE_ROWS: usize = 510;
// The model rejects inputs past ~510 phoneme tokens (its style table has 511
// rows and the ONNX graph errors with "invalid expand shape" beyond that), and
// gets less accurate as it approaches the limit. Synthesis splits text whose
// phoneme stream exceeds this, leaving comfortable margin.
pub(crate) const MAX_TOKENS: usize = 480;

// Per-voice style table: 511 rows of [1][256], indexed by token count.
pub(crate) type VoiceStyles = HashMap<String, Vec<[[f32; STYLE_DIM]; 1]>>;
// (word-or-punct text, token-span start, token-span end) over the token stream.
pub(crate) type WordSpanItem = (String, usize, usize);
pub(crate) type WordMap = Vec<WordSpanItem>;
// One named ONNX session input value.
pub(crate) type SessionInput = (Cow<'static, str>, SessionInputValue<'static>);
