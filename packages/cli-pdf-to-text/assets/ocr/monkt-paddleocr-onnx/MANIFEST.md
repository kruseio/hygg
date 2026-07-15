# monkt/paddleocr-onnx bundled OCR assets

Source: https://huggingface.co/monkt/paddleocr-onnx

Revision: 7b02d0a30a07ba2b92ad1ff5a8941ae2c633de65

License: Apache-2.0. These model assets are redistributed under the
upstream repository license. See https://www.apache.org/licenses/LICENSE-2.0
for the Apache License, Version 2.0 notice and terms.

Bundled files:

| Upstream path | Bundled path | Raw bytes | Raw SHA256 | Gzip bytes | Gzip SHA256 |
| --- | --- | ---: | --- | ---: | --- |
| detection/v3/det.onnx | det.onnx.gz | 2429873 | ee40e80071ba3a320d4efda75f3e22047a7d049e9bf7bcaaf9daea23fc21b935 | 2217343 | e2dac0f04975c28c68624dfa2900d91dd2a10e04be9468176d788dc4c90873a5 |
| languages/english/rec.onnx | rec.onnx.gz | 7830888 | 4e16deb22c4da6468bdca539b2cd3c8687825538b67109177c47d359ab994cd7 | 7228567 | d45c71eef7c4b4d3da4cdfc03beb047807c200732002f1378b9d05678c8d067e |
| languages/english/dict.txt | dict.txt | 1416 | e025a66d31f327ba0c232e03f407ae8d105e1e709e7ccb3f408aa778c24e70d6 | n/a | n/a |

The ONNX assets are stored gzipped to keep the optional bundled OCR
feature close to Cargo's package-size limit. They are decompressed in
memory before constructing `pdf_oxide::ocr::OcrEngine`.
