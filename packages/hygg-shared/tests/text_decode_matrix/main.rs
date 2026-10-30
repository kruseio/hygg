//! Encoding × language matrix for `hygg_shared::text_decode::decode_text`:
//! every corpus sample, encoded in every encoding that can represent it, must
//! decode back to exactly the original text and survive the justifier.

mod codecs;
mod corpus;

use codecs::{Codec, UNICODE};
use hygg_shared::text_decode::decode_text;

/// Corpus pairs whose statistical detection is known to pick a different (but
/// related) code page, as (sample, codec label, encoding picked instead). Each
/// entry is asserted exactly: the bytes must decode as that encoding's reading
/// and must NOT match the original, so the table cannot silently go stale.
/// Empty: with paragraph-length samples chardetng gets every pair right
/// (ISO-8859-2 vs windows-1250, ISO-8859-7 vs windows-1253 including the one
/// letter they disagree on, KOI8-R vs KOI8-U, ...). Genuine ambiguity shows up
/// only on very short inputs; see `very_short_legacy_inputs_can_be_misread`.
const KNOWN_AMBIGUOUS: &[(&str, &str, &str)] = &[];

struct Row {
  sample: &'static str,
  codec: String,
  outcome: Result<&'static str, String>,
}

fn check(sample: &'static str, text: &str, codec: Codec) -> Row {
  let row = |outcome| Row { sample, codec: codec.label(), outcome };
  let (bytes, expected) = match codec.encode(text) {
    Ok(encoded) => encoded,
    Err(e) => return row(Err(format!("sample table bug: {e}"))),
  };
  let decoded = decode_text(&bytes);
  // The justifier must never panic on decoded text, whatever it contains.
  let lines = cli_justify::justify(&decoded.text, 40);
  assert!(!lines.is_empty());

  let ambiguity = KNOWN_AMBIGUOUS
    .iter()
    .find(|(s, c, _)| *s == sample && *c == codec.label());
  if let Some((_, _, detected_as)) = ambiguity {
    let encoding = encoding_rs::Encoding::for_label(detected_as.as_bytes())
      .expect("known-ambiguity encoding label");
    let (as_detected, _) = encoding.decode_without_bom_handling(&bytes);
    let holds = decoded.encoding == encoding.name()
      && decoded.text == as_detected
      && decoded.text != expected;
    return row(if holds {
      Ok("documented ambiguity")
    } else {
      Err(format!(
        "documented ambiguity no longer holds (now detected {})",
        decoded.encoding
      ))
    });
  }
  if decoded.text != expected {
    let at = first_difference(&decoded.text, &expected);
    return row(Err(format!(
      "detected {} — text differs at char {at}",
      decoded.encoding
    )));
  }
  if let Some((encoding, had_bom)) = codec.expected_detection()
    && (decoded.encoding, decoded.had_bom) != (encoding, had_bom)
  {
    return row(Err(format!(
      "text ok but reported {} bom={}",
      decoded.encoding, decoded.had_bom
    )));
  }
  if decoded.lossy {
    return row(Err("text ok but flagged lossy".into()));
  }
  row(Ok(decoded.encoding))
}

fn first_difference(a: &str, b: &str) -> usize {
  a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

#[test]
fn every_sample_round_trips_through_every_encoding_that_can_represent_it() {
  let mut rows = Vec::new();
  for sample in corpus::samples() {
    let text = corpus::load(sample.name);
    for codec in UNICODE.iter().chain(&sample.legacy) {
      rows.push(check(sample.name, &text, *codec));
    }
  }

  // The full matrix, visible with `--nocapture`.
  for row in &rows {
    let result = match &row.outcome {
      Ok(detected) => format!("ok ({detected})"),
      Err(e) => format!("FAIL: {e}"),
    };
    eprintln!("{:<20} {:<14} {result}", row.sample, row.codec);
  }
  let failures: Vec<_> = rows.iter().filter(|r| r.outcome.is_err()).collect();
  eprintln!("{} combinations, {} failed", rows.len(), failures.len());
  assert!(
    failures.is_empty(),
    "{} encoding/language combinations failed: {:?}",
    failures.len(),
    failures
      .iter()
      .map(|r| format!("{}/{}", r.sample, r.codec))
      .collect::<Vec<_>>()
  );
}

/// Statistical detection needs some text to work with. These two-character
/// inputs are genuinely ambiguous and are misdetected; the assertions pin the
/// current reading so a detector change is noticed. (Longer inputs in the same
/// encodings decode correctly — see the matrix above.)
#[test]
fn very_short_legacy_inputs_can_be_misread() {
  use encoding_rs::{BIG5, ISO_8859_2, ISO_8859_5, WINDOWS_874};
  for (text, encoding, read_as) in
    [("ąś", ISO_8859_2, ISO_8859_5), ("中文", BIG5, WINDOWS_874)]
  {
    let (bytes, _, unmappable) = encoding.encode(text);
    assert!(!unmappable);
    let decoded = decode_text(&bytes);
    assert_eq!(decoded.encoding, read_as.name(), "{text}");
    assert_ne!(decoded.text, text);
    // Never a failure, only a wrong guess: the bytes still decode to text.
    assert!(!decoded.text.is_empty());
  }
}

#[test]
fn crlf_line_endings_round_trip_to_lf() {
  for sample in corpus::samples() {
    let text = corpus::load(sample.name);
    let crlf = text.replace('\n', "\r\n");
    for codec in [Codec::Utf8, Codec::Utf8Bom, Codec::Utf16Le { bom: true }]
      .into_iter()
      .chain(sample.legacy.first().copied())
    {
      let (bytes, _) = codec.encode(&crlf).unwrap();
      let (_, expected) = codec.encode(&text).unwrap();
      let decoded = decode_text(&bytes);
      assert_eq!(decoded.text, expected, "{} {}", sample.name, codec.label());
    }
  }
}

#[test]
fn corpus_covers_every_requested_script() {
  let names: Vec<_> = corpus::samples().iter().map(|s| s.name).collect();
  assert_eq!(names.len(), 31);
  for entry in std::fs::read_dir(corpus::corpus_dir()).unwrap() {
    let file = entry.unwrap().file_name().into_string().unwrap();
    let stem = file.trim_end_matches(".txt");
    assert!(names.contains(&stem), "corpus file {file} is not in the table");
  }
}
