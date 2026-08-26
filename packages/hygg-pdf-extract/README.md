## hygg-pdf-extract

A thin fork of [`pdf-extract`](https://crates.io/crates/pdf-extract) `0.10.0`
(by Jeff Muizelaar). The primary change is its `cff-parser` dependency, which
is redirected to [`hygg-cff-parser`](../hygg-cff-parser) — a patched
`cff-parser` that does not `panic!()` on Adobe Expert-encoded CFF fonts.

Published under the `hygg-` namespace so the fix reaches `cargo install hygg`
(a `[patch]` cannot be published).

### Deviations from upstream 0.10.0

Kept to the two below so the fork stays cheap to re-sync. Anything else
belongs upstream, not here.

1. `cff-parser` → `hygg-cff-parser` (the reason the fork exists, above).
2. `lopdf` moved from `0.38` to `0.44`. Upstream 0.10.0 pins `0.38`, which is
   affected by RUSTSEC-2026-0187 (stack overflow on deeply nested PDF
   objects, CVSS 7.5) and has no patched 0.38.x. The bump costs one line of
   source: `Document::get_page_content` returns `Vec<u8>` rather than
   `Result<Vec<u8>>` from 0.39 on, so `output_doc_inner` no longer
   `.unwrap()`s it. This also unifies the workspace on a single `lopdf`,
   since `cli-pdf-to-text` already used 0.43 alongside this crate.

### Retiring this fork

Upstream `pdf-extract 0.12.0` depends on `cff-parser 0.2` (which fixes the
panic) and `lopdf 0.42`, so it clears both deviations above — the condition
this fork was created to wait for. Replacing `hygg-pdf-extract` and
`hygg-cff-parser` with a plain `pdf-extract = "0.12"` dependency is therefore
the intended end state; it is left as a deliberate follow-up because both
crates are published members of the release train.

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
