# monkt/paddleocr-onnx OCR assets

Source: https://huggingface.co/monkt/paddleocr-onnx

Revision: 7b02d0a30a07ba2b92ad1ff5a8941ae2c633de65

License: Apache-2.0. These model assets are redistributed under the
upstream repository license. See https://www.apache.org/licenses/LICENSE-2.0
for the Apache License, Version 2.0 notice and terms.

## Hosting

The models are **not** checked into this repository and are **not** bundled into
the published crate. They are hosted, raw (un-gzipped), as assets on this
project's own GitHub release and downloaded on first use by the `ocr` feature
(see `src/ocr/files.rs`), verified against the pinned SHA256 below, and cached
under the platform cache dir (`HYGG_OCR_MODEL_DIR` overrides the location).

Release: https://github.com/kruseio/hygg/releases/tag/ocr-models-v1.0

| Upstream path | Release asset | Bytes | SHA256 |
| --- | --- | ---: | --- |
| detection/v3/det.onnx | det.onnx | 2429873 | ee40e80071ba3a320d4efda75f3e22047a7d049e9bf7bcaaf9daea23fc21b935 |
| languages/english/rec.onnx | rec.onnx | 7830888 | 4e16deb22c4da6468bdca539b2cd3c8687825538b67109177c47d359ab994cd7 |
| languages/english/dict.txt | dict.txt | 1416 | e025a66d31f327ba0c232e03f407ae8d105e1e709e7ccb3f408aa778c24e70d6 |

The SHA256 values above are the integrity pins hardcoded in `src/ocr/files.rs`;
a download whose size or digest does not match is refused rather than cached.
Re-cutting the release must preserve these exact bytes, or bump the release tag
and the pins together.
