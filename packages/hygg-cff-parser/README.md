# hygg-cff-parser

A thin fork of [`cff-parser`](https://crates.io/crates/cff-parser) `0.1.0` (by
Evgeniy Reizner and Jeff Muizelaar) with a single behavioural fix: upstream
`Encoding::get_table()` calls `panic!()` on Expert-encoded CFF fonts, which
crashes `pdf-extract` on Adobe-generated PDFs. This fork returns the Standard
encoding table for that case instead (see `src/encoding.rs`).

It is published under the `hygg-` namespace purely so the fix reaches
`cargo install hygg`: a `[patch.crates-io]` is workspace-local and stripped on
publish, so it only ever fixed builds from source. Consumed by
`hygg-pdf-extract`. Once upstream `pdf-extract` adopts `cff-parser 0.2` (which
fixes this upstream), both forks can be deleted.

---

This is a fork of the cff1 parsing code from the ttf-parser crate by Evgeniy Reizner.
It adds some of the features that are not used when CFF data is embedded in an OpenType font.

It can be used for parsing CFF data from PDFs.
