//! The language samples (UTF-8 files under test-data/encodings/corpus/) and,
//! for each, the legacy encodings that can represent it. Every sample is also
//! checked in every Unicode form (see `codecs::UNICODE`).

use crate::codecs::Codec::{self, Latin1, Whatwg, Windows1258};
use encoding_rs::*;
use std::path::PathBuf;

pub struct Sample {
  pub name: &'static str,
  pub legacy: Vec<Codec>,
}

fn sample(name: &'static str, legacy: &[Codec]) -> Sample {
  Sample { name, legacy: legacy.to_vec() }
}

pub fn samples() -> Vec<Sample> {
  let western = [Latin1, Whatwg(WINDOWS_1252)];
  let central = [Whatwg(ISO_8859_2), Whatwg(WINDOWS_1250)];
  vec![
    sample("english", &western),
    sample("spanish", &western),
    sample("french", &western),
    sample("german", &western),
    sample("portuguese", &western),
    sample("icelandic", &western),
    sample("polish", &central),
    sample("czech", &central),
    sample("turkish", &[Whatwg(WINDOWS_1254)]),
    sample("greek", &[Whatwg(ISO_8859_7), Whatwg(WINDOWS_1253)]),
    sample(
      "russian",
      &[
        Whatwg(WINDOWS_1251),
        Whatwg(KOI8_R),
        Whatwg(ISO_8859_5),
        Whatwg(IBM866),
      ],
    ),
    sample("ukrainian", &[Whatwg(WINDOWS_1251), Whatwg(KOI8_U)]),
    sample("arabic", &[Whatwg(WINDOWS_1256), Whatwg(ISO_8859_6)]),
    sample("hebrew", &[Whatwg(WINDOWS_1255), Whatwg(ISO_8859_8)]),
    // Persian uses U+06CC (Farsi yeh), which windows-1256 lacks.
    sample("persian", &[]),
    sample("hindi", &[]),
    sample("bengali", &[]),
    sample("tamil", &[]),
    // encoding_rs treats TIS-620 as windows-874, its superset.
    sample("thai", &[Whatwg(WINDOWS_874)]),
    sample("vietnamese", &[Windows1258]),
    sample("chinese_simplified", &[Whatwg(GBK), Whatwg(GB18030)]),
    sample("chinese_traditional", &[Whatwg(BIG5)]),
    sample(
      "japanese",
      &[Whatwg(SHIFT_JIS), Whatwg(EUC_JP), Whatwg(ISO_2022_JP)],
    ),
    sample("korean", &[Whatwg(EUC_KR)]),
    sample("georgian", &[]),
    sample("armenian", &[]),
    sample("amharic", &[]),
    sample("emoji", &[]),
    sample("combining", &[]),
    sample("controls", &[]),
    sample("mixed", &[]),
  ]
}

pub fn corpus_dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../test-data/encodings/corpus")
}

/// The sample text. Panics (rather than skipping) when the corpus is missing:
/// it lives in the repository, so a missing file is a broken checkout.
pub fn load(name: &str) -> String {
  let path = corpus_dir().join(format!("{name}.txt"));
  std::fs::read_to_string(&path)
    .unwrap_or_else(|e| panic!("missing corpus sample {}: {e}", path.display()))
}
