### [<-](../README.md)

## Responsible AI

hygg uses machine learning in a few places, and hygg is built with the help of
AI coding tools. This page says plainly how both work, what happens to your
data, and who is accountable for what ships — so you can decide with the facts
rather than the marketing.

The short version: the AI features run **on your own machine**, your documents
are never uploaded to a model or used to train one, and no line of AI-assisted
code reaches a release without passing automated checks **and** a human review.

### Where AI runs in hygg

Two reader features use neural models, and both run locally:

- **Text-to-speech.** The optional `tts` feature narrates with the open
  [Kokoro](tts.md) voice model through an on-device ONNX runtime. The
  text you narrate is turned into audio on your machine; it is never sent to a
  service. The model weights and voices are downloaded **once** on first use,
  from this project's own GitHub release, over a pinned `sha256` that is
  verified before the file is trusted — there is no third-party origin and no
  silent fallback. After that download the feature works offline.
- **Document extraction and OCR.** Turning a PDF, a scan, or an image into text
  happens locally as part of opening the document. Nothing about the document
  leaves your device for this to work.

Everything else in the reader — navigation, search, highlights, notes — is
ordinary code, no model involved.

### Your documents stay yours

- **No training on your content.** Nothing you read, narrate, highlight, or
  annotate is collected or used to train or fine-tune any model. The models
  hygg ships are fixed, published artifacts; hygg does not learn from you.
- **No telemetry.** The reader does not phone home. The only network traffic is
  the sync you explicitly connect to a server (optional, and self-hostable) and
  the one-time model download above.
- **Offline-first.** Every reading feature, TTS included, works with the network
  off once the model is present. Sync is an addition you opt into, never a
  requirement.

### Human review and quality control

hygg is developed with AI coding assistants. That is a tool for writing code
faster, not a substitute for judgement, and it does not change who is
responsible for the result:

- **Every change is reviewed by a human maintainer** before it merges. Code is
  read, understood, and accepted by a person — never merged because a tool
  produced it. A human is accountable for what ships.
- **Every change passes the gate.** The same checks a contributor runs locally
  guard the tree: formatting, `clippy` with warnings denied, the full test
  suite, and the source-size limit (see [Development](development.md) and
  [Contributing](../../CONTRIBUTING.md)). AI-assisted or hand-written, code earns
  its place the same way — by building, passing the tests, and surviving
  review.
- **Behaviour is verified, not assumed.** Features are exercised end to end and
  covered by tests, so a change is judged by what it actually does, not by what
  it was meant to do.

The standard is deliberately the same for code a person typed and code a tool
helped write: if it cannot be understood, tested, and stood behind by a
maintainer, it does not ship.

### Reporting a concern

If you find a model behaving badly — a bad narration, a mangled extraction — or
have a question about how AI is used here, open an issue on
[GitHub](https://github.com/kruseio/hygg/issues). Reproductions are welcome and
make a fix far faster.
