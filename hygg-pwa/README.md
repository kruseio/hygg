# hygg-pwa

A Rust → WebAssembly Progressive Web App for reading — the touch-first sibling of
the `hygg` terminal reader. It renders the **same hygg justified monospace text**
(reusing `cli-justify`) plus inline ASCII-art images, works **100% offline once
loaded**, and installs to the home screen like a native app. Optimized for iPad
Pro 10.5" but fluid across phones, tablets, and desktops.

Built with [Leptos](https://leptos.dev) (CSR) + [Trunk](https://trunkrs.dev) —
no Node toolchain.

## Direction: one UX, every platform (locked)

**`hygg-pwa` is the canonical reader UX and the basis for every shipped
graphical client** — browser, native desktop, and native mobile. There is one
Leptos frontend; the platforms differ only in the shell around it:

| Target | Shell | Pipeline (extraction/justification) |
| --- | --- | --- |
| Browser (this crate, as-is) | none — served as a PWA | runs in **wasm**, offline |
| Windows / macOS / Linux (GNOME) | **Tauri v2** (`hygg-tauri`) | runs as **native Rust** over Tauri IPC |
| iOS / Android | **Tauri v2** (`hygg-tauri`) | runs as **native Rust** over Tauri IPC |

The native builds wrap **this exact Leptos frontend** in a Tauri v2 shell
(`hygg-tauri`), so the UX is identical everywhere. The one architectural change
on the native targets: the heavy Rust — the `cli-justify` / `cli-epub-to-text` /
`cli-pdf-to-text` extraction pipeline — moves **out of wasm and into native
Tauri IPC commands**, which the UI calls instead of the in-wasm pipeline. That
kills the multi-MB wasm cold-compile tax and runs extraction at native speed
(a real win on weaker mobile CPUs). The browser build keeps the wasm pipeline so
it still extracts fully offline with no server.

**This is where new reader features land.** The native iced client
(`hygg-gui`) is **paused / feature-frozen** — kept as a legacy no-webview
desktop option, to be revisited only once the iced ecosystem (mobile support,
rich text) matures. See `../hygg-gui/README.md`.

## What it does

- **Offline-first.** Import a PDF, EPUB, or text file; it's parsed in the browser
  (the hygg pipeline compiled to wasm), justified, and stored in IndexedDB. After
  the first load a service worker caches the app shell, so reading works with no
  network at all.
- **Touch reading.** A virtualized, smooth-scrolling monospace column. The top bar
  hides on scroll-down for distraction-free reading and returns on scroll-up, with
  a settings gear and back navigation. No keyboard / vim surface.
- **Installable.** A dismiss-once banner offers home-screen install (Chromium
  `beforeinstallprompt`; iOS/iPadOS "Add to Home Screen" instructions).
- **Sync (optional).** Connect a server (defaults to the SaaS) to push/pull
  reading progress and your library across devices — all best-effort, the reader
  never depends on it.
- **Read aloud.** A speaker button narrates the document line-by-line via the Web
  Speech API, highlighting the current line and auto-scrolling to follow along.
- **Settings.** Font size, theme (dark/light/sepia), import column width,
  read-aloud speed, and the sync server URL — persisted to localStorage.

Formats: TXT / Markdown / EPUB / text-PDF are extracted **client-side** (offline).
Scanned (OCR) PDFs and pandoc formats (DOCX/ODT/RTF) are converted on the server
(`POST /api/v1/convert`, entitlement-gated) when connected — a `403` shows an
upgrade nudge. The reader works fully offline for the client-side formats.

## Develop

Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target and
[Trunk](https://trunkrs.dev) (`cargo install --locked trunk`). Trunk downloads a
matching `wasm-bindgen` and `wasm-opt` automatically.

```sh
cargo run -p hygg-pwa                     # dev server + hot reload at http://127.0.0.1:8080
cargo run -p hygg-pwa -- build --release  # production bundle in ./dist
```

`cargo run -p hygg-pwa` is a thin launcher: built for the host target, `main.rs`
just shells out to `trunk` from this crate's directory (no `cd` needed). With no
extra args it runs `trunk serve` (address/port from `Trunk.toml`); everything
after `--` is forwarded to Trunk verbatim. The Leptos/web-sys stack is gated to
the wasm target in `Cargo.toml`, so this host build pulls none of it. Of course
`cd hygg-pwa && trunk serve` / `trunk build --release` still work directly.

The crate is a workspace member but is excluded from `default-members` and from
the host-target CI commands (its real artifact is wasm; the host build is only
the `trunk` launcher). `./tools/ci.sh` covers it with
a dedicated wasm leg: `clippy`/`build`/`udeps` for `wasm32-unknown-unknown`, a
`trunk build`, and an isolation guard asserting `cargo install hygg` never pulls
the Leptos/wasm stack.

## Deploy / hosting

`trunk build --release` emits a fully static bundle in `dist/` (hashed wasm/js/css
plus the unhashed `index.html`, `manifest.webmanifest`, `sw.js`, and `icons/`).
Serve it from any static host at the PWA origin (e.g. `pwa.hygg.kruseio.com`),
with an SPA fallback that serves `index.html` for unknown paths (so deep links
like `/read/:id` work). The bundled service worker also falls back to the cached
shell when offline.

The PWA defaults its server URL to the SaaS server (`https://hygg.kruseio.com`)
and talks to its bearer-token `/api/v1` JSON API; that requires a CORS allow-list
for the PWA origin on the server side (added with the sync work).

## Architecture

```
src/
  main.rs        mount the Leptos app
  app.rs         router shell + global settings context + theme
  routes/        home (library + import) · reader · settings
  components/    top bar (hide-on-scroll) · install prompt
  format.rs      bytes + filename -> extracted, justified Book (reuses pipeline)
  model.rs       Book / BookSummary / Progress / LineKind
  storage.rs     IndexedDB (library / books / progress / blobs) via rexie
  ansi.rs        truecolor ANSI art row -> HTML spans (for PDF image rows)
  settings.rs    preferences persisted to localStorage
index.html       Trunk entry: manifest, icons, SW registration, install shim
manifest.webmanifest, sw.js, styles/main.css, assets/icons/
```

Document identity is `book_id = sha256(source_bytes)` (via `hygg-shared`) — the
same stable key the CLI and server use, so a document here lines up with its
synced twin once server sync is enabled.
