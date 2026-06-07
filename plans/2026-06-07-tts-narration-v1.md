# Plan: Voice narration with word highlighting (v1)

**Date:** 2026-06-07
**Status:** Phase 0 spike ✅ · Phase 1 (UX slice, fake voice) ✅ · Phase 2 (real Kokoro engine behind `tts` feature: ONNX inference + espeak alignment + rodio playback, model downloaded on first use) ✅ — verified: real audio + word-synced highlight. Follow-ups: voice/speed config, download-progress UI, chunk pipelining polish. Results at bottom.
**Roadmap item:** "Natural sounding ai voice model for text to speech narration"

## Scope decisions (locked in)

- **v1 only.** Single machine. No remote audio / SSH routing.
- **Engine: Kokoro-82M via the Kokoros approach** (ONNX inference). **No cloud TTS, no
  cloud anything.** Inference runs locally on the user's machine.
- **Not required to be pure-Rust / no-external-runtime.** Using ONNX Runtime (the `ort`
  crate, which links a native lib) is acceptable for v1. The "all inference in pure Rust"
  roadmap goal is explicitly deferred (would be a later Candle migration).
- **Behind a Cargo feature flag, defaulted off, mirroring the OCR feature.**
- **Hard requirement: the workspace must still build cleanly and publish to crates.io.**

## Non-goals (v1)

- Remote audio / `--audio-sink` / SSH tunnel (was "v3" in the exploration — dropped).
- Cloud engines (ElevenLabs / Polly / Azure) — dropped per request.
- Candle / pure-Rust inference — deferred.
- Multi-language polish, SSML, per-highlight voice switching.

---

## User-facing behavior

- In the reader, `:speak` starts narration from the current position. The currently
  spoken **word** is highlighted; the viewport **auto-scrolls** so the spoken line stays
  centered, advancing line to line as the voice reads.
- `:speak stop` (and any normal-mode key / `Esc`) stops narration.
- Voice and speed come from config (`~/.config/hygg/.env`), e.g. `TTS_VOICE`, `TTS_SPEED`.
- First use downloads the Kokoro model once (see "Model delivery"). Subsequent runs are
  fully offline.
- In a build **without** the feature, `:speak` prints a helpful message
  ("TTS not available in this build; rebuild with `--features tts`"), exactly like the
  OCR stub does today.

---

## The crates.io constraint and the one big decision: download, don't bundle

The OCR feature **bundles** its model into the crate with `include_bytes!`
(`cli-pdf-to-text/src/ocr.rs:11-17`), shipping `det.onnx.gz` (2.1 MB) + `rec.onnx.gz`
(6.9 MB) inside `cli-pdf-to-text`. That is already ~9 MB — right at crates.io's ~10 MiB
default crate-size limit.

**Kokoro-82M is far too large to ship the same way:** ~80 MB (int8/q8) to ~330 MB (fp32),
plus a voices file. Even compressed it blows past the crates.io limit by ~8×, and it would
bloat the git repo.

**Decision: the model is downloaded once on first use and cached locally — NOT vendored
via `include_bytes!`.** This is the only crates.io-compatible option and is standard for
Rust ML crates.

> Note on intent: this is **not** "cloud TTS." No reading data ever leaves the machine.
> It is a one-time fetch of the local model weights (from Hugging Face), after which all
> synthesis is local and offline. To fully satisfy air-gapped / no-network users, we also
> support pre-placing the model manually via an env/config override (see "Model delivery").

So the feature flag mirrors OCR in **structure** (cfg-gated module + stub fallback +
re-export from `hygg`) but differs in **model delivery** (download+cache vs. `include_bytes!`),
purely because of size.

---

## Architecture

```
self.lines (justified display lines)
   │  build WordSpans using the SAME accumulation as persistent_highlight_line_range()
   ▼
Vec<WordSpan{ abs_start, abs_end }>   <- ground-truth on-screen byte offsets (AnsiArt skipped)
   │  chunk by paragraph (synthesize one chunk ahead)
   ▼
Kokoro engine (ort) ── per-chunk ──► audio samples + per-word timings (word,start,end)
   │  align timings[i] -> WordSpan[base+i] sequentially (fuzzy-guarded)
   ▼
worker thread: owns rodio OutputStream+Sink, plays samples, runs a monotonic clock,
   emits SpeechMsg::Word{abs_start,abs_end} at each word's start time
   │  mpsc::Receiver (std)
   ▼
main loop: drain_speech() each tick  (display_loop.rs, next to drain_pdf_stream)
   │   set speech.current = Some((abs_start,abs_end)); move cursor to that (line,col); mark_dirty()
   ▼
renderer: highlight_spoken_word + existing center_cursor() => auto-scroll falls out
```

### Three primitives we reuse (this is why v1 is small)

1. **Background-worker → channel → drain → `mark_dirty`** — copy the PDF streaming model:
   `spawn_loader` (`streaming_loader.rs:31`), `mpsc::sync_channel` (`:39`),
   `drain_pdf_stream` (`core.rs:643`), `mark_dirty` (`core.rs:321`), drained in
   `display_loop.rs:54-66`.
2. **Absolute byte-offset ↔ (line,col) bridge** — `persistent_highlight_line_range`
   (`highlighting_persistent.rs:17`) computes `Σ(lines[i].len()+1)` over non-AnsiArt
   lines. The spoken word is just one live highlight range in that same coordinate space,
   so it reuses the persistent-highlight renderer's logic. **Use byte offsets** (the
   renderer slices `&line[start..end]`).
3. **Cursor-centering auto-scroll** — `center_cursor()` runs each redraw
   (`display_loop.rs:144-150`). Moving `cursor_y`/`offset` to the spoken line makes the
   viewport follow the voice for free. Bonus: progress save already keys off
   `offset + cursor_y` (`display_loop.rs:477`), so position persists while narrating.

### Decoupling "where" from "when"

`WordSpan`s (built from `self.lines`) are the exact on-screen positions. The TTS word
timings only say *when* each successive word is spoken. We align them **sequentially**
(i-th spoken word ↔ i-th `WordSpan`) with a light fuzzy guard for tokenization
mismatches (numbers/punctuation). This avoids fragile string re-search and keeps
highlighting pixel-accurate even if the engine normalizes text.

---

## Feature-flag design

### Where
New feature `tts` on **`cli-text-reader`** (the reader owns the narration UI; OCR lives in
`cli-pdf-to-text` because it's about extraction — TTS is about reading). Re-exported by
`hygg`, exactly like OCR re-exports `pdf-ocr-bundled`.

```toml
# cli-text-reader/Cargo.toml
[features]
default = []
tts = ["dep:ort", "dep:rodio", "dep:<g2p-crate>"]   # heavy deps are optional

[dependencies]
# ... existing ...
ort   = { version = "2", optional = true }            # ONNX Runtime (Kokoro inference)
rodio = { version = "0.20", optional = true }         # audio playback (cpal/CoreAudio)
<g2p-crate> = { version = "...", optional = true }    # grapheme->phoneme (see Phase 0)
# ureq is ALREADY a dependency -> reused for the one-time model download (no new dep)
```

```toml
# hygg/Cargo.toml
[features]
default = []
pdf-ocr-bundled = ["cli-pdf-to-text/pdf-ocr-bundled"]
tts = ["cli-text-reader/tts"]     # NEW, mirrors the OCR re-export
```

### What it gates (and what it does NOT)
Keep the **pure logic ungated and always compiled** so default-feature CI tests cover it:

- **Always compiled (no heavy deps, std only):**
  - `WordSpan` construction + the offset↔(line,col) mapping (reuses existing range fn).
  - Sequential timing→span alignment algorithm (pure fn, unit-tested).
  - Paragraph chunker (pure fn, unit-tested).
  - `SpeechState` (holds `mpsc::Receiver<SpeechMsg>`, cancel flag, worker handle,
    `current: Option<(usize,usize)>`, `playing: bool`) and `SpeechMsg` enum.
  - `drain_speech()` (reads channel, updates highlight + cursor, marks dirty).
  - `highlight_spoken_word_buffered` render branch (cheap; inert when `current==None`).
- **Gated behind `tts` (heavy):**
  - `ort` + G2P + `rodio` usage.
  - The synthesis+playback worker thread (`#[cfg(feature="tts")] start_narration`).
  - Model download/cache.
  - `#[cfg(not(feature="tts"))] start_narration` returns the helpful stub error.

This split means `drain_speech`/renderer have **no `cfg` clutter** (they only ever see a
`SpeechState` that nothing populated when the feature is off), and the tricky logic stays
testable in the normal `cargo test` CI run.

### CI implications (ci.sh)
- `ci.sh` runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  **`--all-features` will compile the `tts` feature**, so the engine code must be
  warning-clean and must build in CI (CI will pull `ort`, which downloads an ONNX Runtime
  binary at build time — confirm CI network allows this in Phase 0).
- `ci.sh` test line has **no** `--all-features`, so unit tests run with TTS off → that's
  why the pure logic must stay ungated (above) to be covered.
- `cargo audit` / `cargo udeps` / `cargo-deny` will scan the new deps — check `ort`,
  `rodio`, and the G2P crate for advisories/licenses (AGPL-3.0 project; verify compatible).
- `.publish.sh` is **unaffected**: publishing uploads source; the model download happens at
  the consumer's runtime, and `ort`'s binary fetch happens at the consumer's build time.
  Default `cargo install hygg` stays lean (feature off).

---

## File-by-file changes

New module: `cli-text-reader/src/editor/speech/` (or `src/speech/`):
- `mod.rs` — `SpeechState`, `SpeechMsg`, `WordSpan`, `build_word_spans(lines, line_kinds)`,
  `align_timings`, `chunk_paragraphs` (all std, always compiled). Unit tests here.
- `engine.rs` — `#[cfg(feature="tts")]` Kokoro/`ort` synthesis: text+voice+speed →
  `(samples, Vec<WordTiming>)`. G2P here.
- `player.rs` — `#[cfg(feature="tts")]` worker thread: owns rodio `OutputStream`+`Sink`
  (not `Send`, so it must be created and live on the worker thread), plays samples, runs
  the monotonic word clock, emits `SpeechMsg` over the channel; honors the `Arc<AtomicBool>`
  cancel.
- `model.rs` — `#[cfg(feature="tts")]` download + cache + checksum + path resolution.

Editor wiring:
- `core_state.rs` — add `pub speech: Option<SpeechState>` to `Editor` (std type, always
  compiled), init `None` in `Editor::new*`.
- `display_loop.rs` — call `self.drain_speech()` near `:54` (just before/after
  `drain_pdf_stream`); re-uses the existing fast-poll cadence while `speech.playing`.
- `core.rs` — add `drain_speech()` (mirror `drain_pdf_stream` `:643`); add
  `#[cfg(feature="tts")] start_narration(from_offset)` and `stop_narration()`; the
  `#[cfg(not)]` `start_narration` returns the stub error.
- `highlighting_persistent.rs` — add a `SpokenWord` arm to `HighlightType` (`:11`) +
  a `highlight_spoken_word_buffered` (reverse-video / bright bg), invoked from
  `draw_content_buffered` (`display.rs:153`) before persistent highlights. Inert when
  `speech` is `None`/`current==None`.

Command surface (mirror OCR exactly):
- `command_registry.rs` — add `Speak(SpeakAction)` to `RegisteredCommand` (`:8`);
  `SPEAK_ARGS = &["stop"]`; `CommandSpec { name: "speak", arguments: SPEAK_ARGS }` in
  `COMMANDS` (`:42`); arms in `classify_command` (`:85`): `("speak", []) => Speak(Start)`,
  `("speak", ["stop"]) => Speak(Stop)`; a line in `command_help_lines` (`:115`).
- `commands.rs` — add `RegisteredCommand::Speak(action) => self.handle_speak_command(action)`
  to the dispatch match (`:160-193`).
- `commands_handlers.rs` — add `handle_speak_command(action)` modeled on
  `handle_ocr_command` (`:48`): start/stop narration, reset mode, clear command buffer,
  `mark_dirty()`.

Config (mirror `pdf_ocr` in `config.rs`):
- Add `tts_voice: Option<String>` and `tts_speed: Option<f32>` to `AppConfig` (`:9-16`);
  defaults in `ensure_config_file` (`:26`); load in `load_config` (`:40-46`); persist in
  `save_config` (`:68-84`). (String/float, so add small `config_string`/`config_f32`
  helpers next to `config_bool` `:52`.)

CLI (optional, nice-to-have): add `--speak` to `hygg/src/args.rs` (start narration on
open) routed in `hygg/src/main.rs` next to the OCR routing (`:46-51`). Can ship after the
`:speak` command.

---

## Model & voice delivery

- **Model:** `onnx-community/Kokoro-82M-v1.0-ONNX-timestamped` — the **timestamped**
  variant is what yields per-word timings. Prefer the quantized `*_q8` (~80 MB) for v1 to
  keep the download small; allow fp32 via config later.
- **Voices:** the Kokoro voices file (e.g. `voices-v1.0.bin`). Default voice from config
  (`TTS_VOICE`, e.g. an `af_*`/`am_*` id); document the available ids.
- **Cache location:** `dirs::cache_dir()/hygg/tts/` (fallback `~/.config/hygg/tts/`).
- **Integrity:** verify SHA-256 against pinned hashes; re-download on mismatch.
- **Download UX:** first `:speak` shows a one-time "Downloading voice model… X%" using the
  existing status-line / loading-splash machinery; uses `ureq` (already a dependency).
- **Offline / air-gap override:** `HYGG_TTS_MODEL_DIR` env var (or a config key) pointing at
  a pre-placed model dir → skip download entirely. Documented for no-network users.

---

## Phased tasks

**Phase 0 — Spike (de-risk before committing).** *Deliverable: a throwaway binary that
synthesizes a paragraph and prints per-word timings.*
- Pick the dependency stack and confirm it as a **library** (not just a CLI):
  evaluate `kokoros`/`kokoro-tts`/`kokoroxide` for (a) library API, (b) **per-word
  timings exposed programmatically**, (c) G2P that is **pure-Rust** (prefer `misaki-rs` /
  `voice-g2p`) to avoid an espeak-ng C dependency that could hurt "build cleanly."
- Confirm `ort` builds cleanly via `cargo build --features tts` on macOS (M-series) and
  Linux, and in CI (network for the ONNX Runtime binary download).
- If no crate exposes timings as a library: fall back to reading the timestamped model's
  per-token durations directly. **If this phase shows timings aren't reliably available,
  stop and reassess** (timings are the whole feature).

**Phase 1 — Pure logic (no feature needed).** `WordSpan` building, alignment, chunker,
`SpeechState`/`SpeechMsg`, `drain_speech`, the `SpokenWord` render branch. Drive it with a
**fake engine** that emits synthetic timings so the highlight + auto-scroll can be seen and
tested with zero ML deps. Unit tests for mapping/alignment/chunking.

**Phase 2 — Real engine + playback (gated).** `engine.rs` (Kokoro/ort + G2P),
`player.rs` (rodio worker + clock), `model.rs` (download/cache). Wire `start/stop_narration`.

**Phase 3 — Command + config surface.** `:speak` / `:speak stop`, config keys, help text,
`classify_command` tests (mirror the OCR tests at `commands.rs:219`).

**Phase 4 — Polish.** Synthesize-one-chunk-ahead pipelining, pause/resume, optional
"already-read" dimming trail, `--speak` CLI flag, docs page, README roadmap checkbox.

---

## Testing

- **Unit (run in default CI):** `build_word_spans` byte-offset correctness incl. AnsiArt
  skipping; `align_timings` with deliberately mismatched tokenization; chunker boundaries.
- **Integration with fake engine (default CI):** feed synthetic timings → assert
  `speech.current` advances and `cursor_y`/`offset` track the spoken line (auto-scroll).
- **Feature build (clippy `--all-features`):** ensures `engine/player/model` compile clean.
- **Manual:** real narration on a PDF and an EPUB on the M4; verify highlight/voice sync,
  scroll smoothness, stop/restart, offline run after first download.

---

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| Kokoro crate doesn't expose per-word timings as a library | Phase 0 gate; fall back to reading timestamped-model token durations; reassess if absent |
| `ort` build-time ONNX Runtime download fails (CI/firewall) | Document; confirm in Phase 0; consider `load-dynamic` as documented alternative |
| espeak-ng C dependency sneaks in via a TTS crate → "build cleanly" risk | Prefer pure-Rust G2P (`misaki-rs`/`voice-g2p`); verify no native G2P dep |
| Model too big for crates.io | Resolved: download-on-first-run, never `include_bytes!` |
| License: project is AGPL-3.0 | Verify `ort`/`rodio`/G2P + model license compatibility via `cargo-deny` |
| Timing drift over long reads | Re-anchor the clock per chunk (we synthesize per paragraph anyway) |
| Multibyte slicing panics | Use byte offsets throughout (matches persistent-highlight slicing) |

---

## Open questions (defer; not blocking the plan)

- Default voice id and whether to ship q8 (smaller) vs fp32 (nicer) first → **q8**.
- A normal-mode keybinding to toggle narration (in addition to `:speak`)? Which key?
- Should `:speak` start at the cursor or at the top of the viewport? → **cursor line.**
- Highlight style for the spoken word (reverse video vs. bright bg) + optional read-trail.

---

## Phase 0 spike results (2026-06-07)

**Verdict: GREEN.** Per-word timings are real, good quality, and fast on the M4. The
feature is feasible. The spike changed two implementation decisions (see "Revised
decisions" below).

Spike location: `/tmp/hygg-tts-spike/` (throwaway, ~1.1 GB incl. model + build target —
`rm -rf /tmp/hygg-tts-spike` to reclaim). Built the upstream `koko`/`kokoros`
(github.com/lucasjinreal/Kokoros) and ran the *timestamped* model end-to-end.

### What was proven
- **Per-word timings work**, both via the `koko --timestamps` TSV and the underlying
  library call `TTSKoko::tts_timestamped_raw_audio(...)` which returns
  `Vec<WordAlignment { word: String, start_sec: f32, end_sec: f32 }>`
  (`kokoros/src/tts/koko.rs:27,744`). This retires the plan's biggest risk.
- **Timing quality is correct**: monotonic, sentence `.` and comma `,` appear as their own
  rows (pauses), durations look right. Sample (paragraph with a number + a made-up word):
  ```
  word        start_sec  end_sec
  The         0.000      0.113
  dog         2.384      3.430
  .           3.430      3.711
  effortless  4.604      5.143
  2026        8.908     10.857   <- kept as on-screen token "2026" (spoken ~2s), NOT phonetics
  hygg       10.997     11.388   <- unknown word still aligns
  speak      11.880     12.894
  ```
- **Original word tokens are preserved** in the alignment (numbers/odd words map straight
  back to the on-screen text — no de-normalization needed for highlighting).
- **Performance**: 3.85 s to render ~13 s of audio on the M4 CPU (~3.4× realtime) with the
  fp32 model, single instance. Synth-ahead-by-paragraph is comfortably viable.
- **Model**: the *timestamped* variant is required (strategy = ">1 ONNX output";
  `ort_koko.rs:57`). Downloaded `onnx-community/Kokoro-82M-v1.0-ONNX-timestamped`
  `onnx/model.onnx` (310 MB fp32) + `voices-v1.0.bin` (27 MB) from HF at runtime. Output is
  24 kHz. (q8 variants exist to shrink the download for v1.)
- **Audio** rendered fine (sent the WAV to the user to judge naturalness). Voice used:
  `af_sarah.4+af_nicole.6` blend.

### Packaging findings (these are the important ones)
1. **`kokoros` is NOT published on crates.io (404).** Therefore hygg cannot depend on it —
   `cargo publish` strips path/git deps. **We must port the minimal logic, not depend on
   the crate.**
2. **`espeak-rs` / `espeak-rs-sys` ARE published** (0.1.5–0.2.0). `espeak-rs-sys` **vendors
   the full espeak-ng source + data (28 MB) and builds it with CMake** — so it's
   self-contained (no system espeak-ng at build or runtime) BUT it pulls in a **cmake + C/C++
   toolchain** build requirement.
3. **CMake 4.x gotcha (hit during the spike):** vendored CMake builds that declare
   `cmake_minimum_required(<3.5)` fail on cmake ≥ 4.0 ("Compatibility with CMake < 3.5 has
   been removed"). This bit `audiopus_sys` and applies to `espeak-rs-sys`/espeak-ng too.
   Workaround: `CMAKE_POLICY_VERSION_MINIMUM=3.5`. Also needed `pkg-config` installed.
   → **This will bite end users of `cargo install hygg --features tts`** until upstream
   bumps the minimum; must be documented (and ideally mitigated, see below).
4. **`ort` downloads a prebuilt ONNX Runtime binary at build time** (network needed at
   build). Fine, but note for offline/firewalled builders.
5. Default `cargo install hygg` (feature OFF) is **completely unaffected** by all of the
   above — the heavy/native surface only exists under `--features tts`.

### Revised decisions (supersede the speculative parts above)
- **Port, don't depend.** Vendor a minimal slice into hygg's `tts` feature:
  - inference: `kokoros/src/onn/ort_koko.rs` (model load, timestamped-strategy detection,
    `infer()` → audio + per-token `durations`)
  - alignment: `kokoros/src/tts/koko.rs` (per-word phoneme tokenization →
    duration→`WordAlignment`, frames @ 40/sec, punctuation pauses)
  - Kokoros is Apache-2.0 → compatible with this AGPL-3.0 project (one-way).
- **Lean dependency set** (all crates.io-published): `ort`, `espeak-rs`, `ndarray`,
  `ndarray-npy`, plus `rodio` for playback. **Drop** what `kokoros` pulls but hygg doesn't
  need: `opus`, `ogg`, `mp3lame-encoder`, `reqwest`, `tokio`, `uuid`, `indicatif`
  (hygg needs raw f32 PCM for rodio — no encoders, no async HTTP; model download uses the
  existing `ureq`).
- **G2P fork to decide before/at Phase 2:** keeping espeak (`espeak-rs`) means a
  cmake + C-toolchain build requirement for the `tts` feature. If we want a *truly*
  dependency-light, cmake-free build, swap to a **pure-Rust G2P** (`misaki-rs` /
  `voice-g2p`) and re-implement the per-word token-span alignment on top of it. Tradeoff:
  espeak path = proven-now but heavier build; pure-Rust path = cleaner build but more work
  and needs its own alignment validation. **Recommendation:** prototype Phase 1 with the
  espeak port (proven), and evaluate the pure-Rust G2P swap before first release.

### Risk outcomes
| Risk (from plan) | Outcome |
|---|---|
| Timings exposed as a library? | ✅ Yes (`WordAlignment` + `tts_timestamped_raw_audio`) |
| `ort` builds cleanly? | ✅ Yes on M4 (downloads ONNX RT binary at build) |
| espeak-ng C dependency? | ⚠️ Real: vendored+cmake via `espeak-rs-sys` → needs cmake/C toolchain; pure-Rust G2P is the escape hatch |
| Model too big to bundle | ✅ Confirmed (310 MB fp32) → runtime download stands |
| Depend on `kokoros`? | ❌ Not published on crates.io → **port instead** |
| New: CMake 4.x vendored-build break | ⚠️ Needs `CMAKE_POLICY_VERSION_MINIMUM=3.5`; document + watch upstream |

### Repro (M4, macOS) for whoever picks this up
Prereqs: `brew install espeak-ng cmake pkgconf opus`. Then:
```
git clone --depth 1 https://github.com/lucasjinreal/Kokoros /tmp/hygg-tts-spike/Kokoros
cd /tmp/hygg-tts-spike/Kokoros
curl -L -o checkpoints/model.onnx \
  https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX-timestamped/resolve/main/onnx/model.onnx
curl -L -o data/voices-v1.0.bin \
  https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin
CMAKE_POLICY_VERSION_MINIMUM=3.5 cargo build --release -p koko
ESPEAKNG_DATA_PATH=/opt/homebrew/share/espeak-ng-data \
  ./target/release/koko -m checkpoints/model.onnx --instances 1 --timestamps \
  text -o tmp/output.wav "Your paragraph here."
cat tmp/output.tsv
```
