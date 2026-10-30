//! The decoded text of a document, and the identity its saved state is keyed
//! by.

use hygg_shared::text_decode::{DecodedText, decode_text, normalize_text};

/// A document's text in the two forms the reader needs.
pub(crate) struct Content {
  /// What is justified and shown: decoded from whatever encoding the source
  /// used, BOM stripped, line endings normalised to LF.
  pub(crate) display: String,
  /// What the document's saved state is keyed by. For input that hygg could
  /// already open before it decoded encodings (valid UTF-8 bytes, other
  /// converters' output) this is that same text byte-for-byte — BOM and CRLF
  /// included — so existing progress, highlights, bookmarks, notes and the
  /// sync `book_id` stay attached to the document. Saved positions that
  /// pointed into the old layout are moved by the reader on first open (see
  /// cli-text-reader's `runner/layout_migration.rs`). Input that used to fail
  /// (UTF-16, legacy code pages) has no saved state, so it uses `display`.
  pub(crate) identity: String,
  /// The encoding detected in the source bytes (e.g. `windows-1252`,
  /// `UTF-8 (BOM)`), kept for the debug log so it needn't decode again.
  /// `None` for text another converter already decoded.
  pub(crate) encoding: Option<String>,
}

impl Content {
  /// Decode the raw bytes of a plain-text source (a `.txt` file or stdin).
  /// Exits with an error for a signature-identified encoding that cannot be
  /// decoded (UTF-1, UTF-EBCDIC, SCSU, BOCU-1) rather than showing mojibake.
  pub(crate) fn from_bytes(bytes: Vec<u8>, source: &str) -> Self {
    let decoded = decode_text(&bytes);
    if let Some(message) = unsupported_encoding_error(&decoded, source) {
      eprintln!("{message}");
      std::process::exit(1);
    }
    let encoding = Some(format!(
      "{}{}{}",
      decoded.encoding,
      if decoded.had_bom { " (BOM)" } else { "" },
      if decoded.lossy { " (lossy)" } else { "" }
    ));
    let identity =
      String::from_utf8(bytes).unwrap_or_else(|_| decoded.text.clone());
    Self { display: decoded.text, identity, encoding }
  }

  /// Text another converter produced (EPUB, pandoc): already a string, but it
  /// may still carry a leading BOM or CRLF line endings.
  pub(super) fn from_converted(text: String) -> Self {
    Self { display: normalize_text(&text), identity: text, encoding: None }
  }

  /// PDF text, shown exactly as extracted (the PDF justifier handles it).
  pub(super) fn verbatim(text: String) -> Self {
    Self { display: text.clone(), identity: text, encoding: None }
  }
}

pub(crate) fn unsupported_encoding_error(
  decoded: &DecodedText,
  source: &str,
) -> Option<String> {
  decoded.unsupported().then(|| {
    format!(
      "Error:\nUnable to read {source}\n\nDetails:\nThe text is encoded in \
       {}, which hygg cannot decode. Convert it to UTF-8 and try again.\n",
      decoded.encoding
    )
  })
}
