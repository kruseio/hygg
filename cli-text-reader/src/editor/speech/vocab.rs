// Kokoro phoneme vocabulary + tokenizer.
//
// Ported verbatim from the Kokoros project (Apache-2.0): the exact symbol set
// and ordering the Kokoro ONNX model was trained against. `tokenize` maps an
// IPA phoneme string (from espeak-ng) to the model's input token ids by
// looking up each character; unknown characters are dropped, matching upstream.

use std::collections::HashMap;
use std::sync::OnceLock;

fn build_vocab() -> HashMap<char, usize> {
  let pad = "$";
  let punctuation = ";:,.!?¡¿—…\"«»“” ";
  let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
  let letters_ipa = "ɑɐɒæɓʙβɔɕçɗɖðʤəɘɚɛɜɝɞɟʄɡɠɢʛɦɧħɥʜɨɪʝɭɬɫɮʟɱɯɰŋɳɲɴøɵɸθœɶʘɹɺɾɻʀʁɽʂʃʈʧʉʊʋⱱʌɣɤʍχʎʏʑʐʒʔʡʕʢǀǁǂǃˈˌːˑʼʴʰʱʲʷˠˤ˞↓↑→↗↘'̩'ᵻ";

  let symbols: String = [pad, punctuation, letters, letters_ipa].concat();
  symbols.chars().enumerate().map(|(idx, c)| (c, idx)).collect()
}

fn vocab() -> &'static HashMap<char, usize> {
  static VOCAB: OnceLock<HashMap<char, usize>> = OnceLock::new();
  VOCAB.get_or_init(build_vocab)
}

/// Map an IPA phoneme string to Kokoro input token ids (unknown chars dropped).
pub(super) fn tokenize(phonemes: &str) -> Vec<i64> {
  phonemes
    .chars()
    .filter_map(|c| vocab().get(&c))
    .map(|&idx| idx as i64)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tokenize_matches_known_vector() {
    // "Hello!" -> these ids fixes the symbol ordering (H=24, e=47, l=54,
    // o=57, !=5). This is the inverse of the upstream tokens_to_phonemes test.
    assert_eq!(tokenize("Hello!"), vec![24, 47, 54, 54, 57, 5]);
    assert!(tokenize("").is_empty());
    assert_eq!(tokenize("..."), vec![4, 4, 4]); // '.' = id 4
  }
}
