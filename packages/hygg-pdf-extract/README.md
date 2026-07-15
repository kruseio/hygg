## hygg-pdf-extract

A thin fork of [`pdf-extract`](https://crates.io/crates/pdf-extract) `0.10.0`
(by Jeff Muizelaar). The **only** change is its `cff-parser` dependency, which
is redirected to [`hygg-cff-parser`](../hygg-cff-parser) — a patched
`cff-parser` that does not `panic!()` on Adobe Expert-encoded CFF fonts. The
source is otherwise verbatim, so it can be re-synced from upstream with a
one-line manifest change.

Published under the `hygg-` namespace so the fix reaches `cargo install hygg`
(a `[patch]` cannot be published). Once upstream `pdf-extract` moves to
`cff-parser 0.2` (which fixes the panic), this fork can be deleted.

---

## pdf-extract
[![Build Status](https://github.com/jrmuizel/pdf-extract/actions/workflows/rust.yml/badge.svg)](https://github.com/jrmuizel/pdf-extract/actions)
[![crates.io](https://img.shields.io/crates/v/pdf-extract.svg)](https://crates.io/crates/pdf-extract)
[![Documentation](https://docs.rs/pdf-extract/badge.svg)](https://docs.rs/pdf-extract)

A rust library to extract content from PDF files.

```rust
let bytes = std::fs::read("tests/docs/simple.pdf").unwrap();
let out = pdf_extract::extract_text_from_mem(&bytes).unwrap();
assert!(out.contains("This is a small demonstration"));
```

## See also

- https://github.com/elacin/PDFExtract/
- https://github.com/euske/pdfminer / https://github.com/pdfminer/pdfminer.six
- https://gitlab.com/crossref/pdfextract
- https://github.com/VikParuchuri/marker
- https://github.com/kermitt2/pdfalto used by [grobid](https://github.com/kermitt2/grobid/)
- https://github.com/opendatalab/MinerU (uses PyMuPDF and pdfminer.six)

### Not PDF specific
- https://github.com/Layout-Parser/layout-parser
