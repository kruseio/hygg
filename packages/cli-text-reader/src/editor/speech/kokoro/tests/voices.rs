use std::io::{Cursor, Write};

use ndarray::{Array2, Array3};
use ndarray_npy::WriteNpyExt;
use zip::write::SimpleFileOptions;

use super::super::common::STYLE_DIM;
use super::super::files::load_voices;

fn archive(invalid_shape: bool) -> Vec<u8> {
  let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
  for (name, method) in [
    ("af_test.npy", zip::CompressionMethod::Deflated),
    ("bf_test", zip::CompressionMethod::Stored),
  ] {
    zip
      .start_file(name, SimpleFileOptions::default().compression_method(method))
      .unwrap();
    if invalid_shape {
      Array2::<f32>::zeros((2, STYLE_DIM)).write_npy(&mut zip).unwrap();
    } else {
      let array =
        Array3::from_shape_fn((2, 1, STYLE_DIM), |(i, _, k)| (i + k) as f32);
      array.write_npy(&mut zip).unwrap();
    }
  }
  zip.finish().unwrap().into_inner()
}

fn load(bytes: &[u8]) -> Result<super::super::common::VoiceStyles, String> {
  let mut file = tempfile::NamedTempFile::new().unwrap();
  file.write_all(bytes).unwrap();
  load_voices(file.path())
}

#[test]
fn loads_compressed_and_stored_voices_with_optional_npy_suffix() {
  let voices = load(&archive(false)).unwrap();
  assert_eq!(voices.len(), 2);
  for name in ["af_test", "bf_test"] {
    assert_eq!(voices[name][1][0][10], 11.0);
    assert_eq!(voices[name][510][0][10], 0.0);
  }
}

#[test]
fn rejects_cloaked_voice_names_and_invalid_array_shapes() {
  let mut bytes = archive(false);
  bytes[30] = b'X'; // Local header now disagrees with the central directory.
  let error = load(&bytes).unwrap_err();
  assert!(error.contains("header"), "{error}");
  assert!(load(&archive(true)).is_err());
}
