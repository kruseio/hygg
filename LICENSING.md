# Licensing

hygg is a **multi-license workspace**. There is no single project-wide license:
each crate declares its own license in its `Cargo.toml`, and that declaration is
authoritative. This document explains what is licensed how, and why.

- **Reading documents with hygg, on any of its clients** — AGPL-3.0-only.
- **Building on the libraries** (converters, justifier, shared types) — MIT.
- **Self-hosting the sync server** — Elastic License 2.0 (source-available).
  You may run it for yourself or your organisation; you may not offer it to
  third parties as a managed service.

## License texts

All four license texts live in the repository root:

| File | License | SPDX |
|---|---|---|
| [`LICENSE`](LICENSE) | GNU Affero General Public License v3.0 | `AGPL-3.0-only` |
| [`LICENSE-MIT`](LICENSE-MIT) | MIT License | `MIT` |
| [`LICENSE-APACHE`](LICENSE-APACHE) | Apache License 2.0 | `Apache-2.0` |
| [`LICENSE-ELASTIC`](LICENSE-ELASTIC) | Elastic License 2.0 | `Elastic-2.0` |

`LICENSE` is the AGPL text — it is unsuffixed so GitHub detects and labels the
repository, whose flagship product (the reader) is AGPL. That label describes
the reader, not every crate; use the table below for anything else.

Some crates also carry their own copy, because the license text has to travel
inside the published package:

| Crate file | Relationship to the root text |
|---|---|
| [`hygg-server/LICENSE`](hygg-server/LICENSE) | Identical to [`LICENSE-ELASTIC`](LICENSE-ELASTIC); referenced by that crate's `license-file` key. |
| [`hygg-cff-parser/LICENSE-APACHE`](hygg-cff-parser/LICENSE-APACHE) | Identical to [`LICENSE-APACHE`](LICENSE-APACHE). |
| [`hygg-cff-parser/LICENSE-MIT`](hygg-cff-parser/LICENSE-MIT) | **Deliberately different** — the upstream MIT text, © 2018 Yevhenii Reizner. |
| [`hygg-pdf-extract/LICENSE`](hygg-pdf-extract/LICENSE) | **Deliberately different** — the upstream MIT text, © 2014 Jeff Muizelaar and the pdf-extract contributors, plus kruseio's copyright on the fork modifications. |

The forks' copies name their upstream copyright holders rather than kruseio;
that is the point of a fork retaining its original license, so keep those
notices intact.

## Crates

Every crate in the workspace, and what covers it:

| Crate | Kind | Published | License | Why |
|---|---|---|---|---|
| [`cli-justify`](cli-justify) | Library | crates.io | **MIT** | Reusable text justifier — permissive so anyone can build on it. |
| [`cli-image-to-ascii`](cli-image-to-ascii) | Library | crates.io | **MIT** | Reusable truecolor ANSI image renderer. |
| [`cli-epub-to-text`](cli-epub-to-text) | Library / CLI | crates.io | **MIT** | Reusable EPUB→text converter. |
| [`cli-pdf-to-text`](cli-pdf-to-text) | Library / CLI | crates.io | **MIT** | Reusable PDF→text converter (incl. optional OCR). |
| [`redirect-stderr`](redirect-stderr) | Library | crates.io | **MIT** | Small cross-platform stderr utility. |
| [`hygg-shared`](hygg-shared) | Library | crates.io | **MIT** | Shared types, including the `sync::proto` client/server wire contract. Permissive so it can sit on both sides of that boundary. |
| [`cli-text-reader`](cli-text-reader) | Library | crates.io | **AGPL-3.0-only** | The reader/TUI engine — network copyleft keeps the product and its hosted forks open. |
| [`hygg`](hygg) | Binary | crates.io | **AGPL-3.0-only** | The shipped CLI, on top of `cli-text-reader`. |
| [`hygg-gui`](hygg-gui) | Binary | no (`publish = false`) | **AGPL-3.0-only** | The same reader product in a native iced desktop shell. |
| [`hygg-pwa`](hygg-pwa) | Binary / WASM | no (`publish = false`) | **AGPL-3.0-only** | The same reader product as a Rust/WASM Leptos PWA. Served over a network, so AGPL is the point. |
| [`hygg-tauri`](hygg-tauri) | Binary | no (`publish = false`) | **AGPL-3.0-only** | Native Tauri v2 desktop/mobile shell over the `hygg-pwa` UI — same product, same license. |
| [`hygg-server`](hygg-server) | Library / Binary | crates.io | **Elastic License 2.0** | The sync server — source-available. Self-host and modify freely; do not offer it to third parties as a managed service. ELv2 is **not** an OSI "open source" license. |
| [`hygg-cff-parser`](hygg-cff-parser) | Library | crates.io | **MIT OR Apache-2.0** | Thin fork of `cff-parser` — retains the upstream dual license. |
| [`hygg-pdf-extract`](hygg-pdf-extract) | Library | crates.io | **MIT** | Thin fork of `pdf-extract` — retains the upstream license. |

The three `publish = false` clients (`hygg-gui`, `hygg-pwa`, `hygg-tauri`) are
not on crates.io, but they ship to users as built binaries and hosted apps —
their AGPL terms apply to those distributions exactly as they do to `hygg`.

### Everything else in the repository

Documentation (`docs/`, `*.md`), assets (`assets/`), CI workflows and scripts
(`.github/`, `tools/`) are part of the hygg project and are covered by
**AGPL-3.0-only** unless the file itself says otherwise. The exceptions are the
third-party materials listed below.

## Why the split is dependency-safe

The licenses do not leak into each other, and this is enforced by dependency
direction rather than by convention:

- **Every MIT crate depends only on permissive crates.** No AGPL or Elastic code
  is reachable from them, so they stay genuinely reusable.
- **The AGPL clients sit at the top of the tree.** `hygg`, `hygg-gui`,
  `hygg-pwa` and `hygg-tauri` are leaves that nothing else depends on; they may
  absorb their MIT dependencies, which MIT permits.
- **The Elastic server shares no code with the AGPL clients.** `hygg-server`
  depends only on `hygg-shared`, `cli-pdf-to-text`, `cli-epub-to-text` and
  `cli-justify` — all MIT. It does not depend on `cli-text-reader` or any other
  AGPL crate.
- **The client/server boundary is a wire contract, not shared code.** Both sides
  are statically typed against **MIT** `hygg-shared::sync::proto`, so no license
  crosses the boundary in either direction.

One practical consequence: `cargo install hygg` never pulls the server's
axum/tokio/sea-orm stack, and never pulls iced, Tauri or Leptos. The isolation
is structural, not a packaging flag.

## Third-party material

### Bundled with the code

| What | Where | License | Notes |
|---|---|---|---|
| PaddleOCR ONNX models | [`cli-pdf-to-text/assets/ocr/monkt-paddleocr-onnx/`](cli-pdf-to-text/assets/ocr/monkt-paddleocr-onnx) | **Apache-2.0** | Redistributed under the upstream repository's license. Only compiled in with the optional `pdf-ocr-bundled` feature. Provenance, revision and checksums are in that directory's [`MANIFEST.md`](cli-pdf-to-text/assets/ocr/monkt-paddleocr-onnx/MANIFEST.md). |
| Fira Mono (Medium) | [`hygg-gui/assets/fonts/`](hygg-gui/assets/fonts) | **SIL Open Font License 1.1** | Digitized data © 2012–2015 The Mozilla Foundation and Telefónica S.A. Full text: [`FiraMono-LICENSE`](hygg-gui/assets/fonts/FiraMono-LICENSE). The OFL covers the font file only and does not affect the license of the software that embeds it. |

### Pulled in by the optional `tts` feature

The `tts` feature is **off by default** — neither `cargo install hygg` nor a
default build pulls any of it. If you enable it, note:

- **espeak-ng is GPL-3.0-or-later, and it is statically linked.** The
  `espeak-rs` wrapper crate is MIT, but it vendors the espeak-ng C sources and
  builds them into the binary. A distributed `--features tts` build therefore
  carries GPL-3.0-or-later obligations for that component in addition to hygg's
  own AGPL terms. The two are compatible — GPLv3 §13 expressly permits combining
  with AGPLv3 work — but if you redistribute such a build, the GPL obligations
  (notably offering corresponding source) are yours to meet.
- **ONNX Runtime** is fetched at build time by `ort` (`ort` and ONNX Runtime are
  permissively licensed; `ort` itself is MIT OR Apache-2.0).
- **The Kokoro voice model is not redistributed by hygg.** The model
  (`Kokoro-82M-v1.0-ONNX-timestamped`, from the `onnx-community` Hugging Face
  repository) and the `voices-v1.0.bin` voice file (from the `kokoro-onnx`
  releases) are downloaded from upstream on first use and remain governed by
  their own upstream terms. Check those terms before redistributing the model
  files or shipping a product built around them.

### Test fixtures

`test-data/` contains third-party documents used purely as parser fixtures —
among them the Adobe PDF Reference and Pro Git. **These retain their own
copyright and terms and are not licensed under any of the project's licenses.**
They are included for testing only; do not treat their presence here as a grant
of rights, and do not redistribute them as part of a derived product.

## What this means for you

- **Reading documents with hygg.** Use it however you like, privately, at no
  cost, with no obligations. Copyleft is about distribution, not use.
- **Modifying a client and keeping it to yourself.** Fine — the AGPL only
  triggers on distribution or on offering network access to the modified
  program.
- **Distributing a modified client, or hosting one for others to use.** The AGPL
  applies: publish your corresponding source under AGPL-3.0-only. This is the
  case the license exists for.
- **Building on the MIT libraries.** Take them: keep the copyright notice and
  the MIT text, and you are done. Nothing obliges you to open your own code.
- **Self-hosting `hygg-server`.** Run it for yourself, your team or your
  company, and modify it as you need. What ELv2 forbids is providing it to third
  parties as a hosted or managed service, and removing or circumventing its
  license-key functionality.
- **Offering hygg as a commercial service.** That needs a separate agreement —
  see below.

## Contributions

Contributions are accepted under the project's [Contributor License
Agreement](CLA.md), which you accept implicitly by submitting a pull request; no
signature is required. It lets the maintainer license and relicense
contributions — including under commercial terms — which is what makes this
multi-license structure, and any future commercial licensing, possible.

Keep changes within the license of the crate(s) you touch: do not move
AGPL-licensed code into an MIT crate, and do not import AGPL code into
`hygg-server`. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Commercial licensing

The AGPL and ELv2 terms above do not fit every use — commercial and OEM
licenses, including terms that lift the AGPL's source-publication requirement
or ELv2's managed-service restriction, are available from kruseio. Open an
issue or contact the maintainer at
[github.com/kruseio/hygg](https://github.com/kruseio/hygg) to discuss.

---

*This document explains the project's licensing in plain language. Where it and
a license text disagree, the license text and the crate's `Cargo.toml`
declaration control. It is not legal advice.*
