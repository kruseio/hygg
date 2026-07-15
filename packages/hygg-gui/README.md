# hygg-gui

> **Status: paused — legacy client (feature-frozen).** Active development of the
> reader has moved to `hygg-pwa` + a Tauri v2 native shell (`hygg-tauri`), which
> ships the same UX to native **desktop *and* mobile** from one codebase. This
> iced crate stays in the tree as a fully-native, no-webview desktop option and
> a UX reference, but **no new features land here**. It will be revisited only
> once the iced ecosystem matures enough for our needs — notably **mobile
> targets** (iced has no production iOS/Android story today) and richer text
> handling — at which point native-native (no webview) becomes worth carrying
> forward again. Until then: mirror-only, no new surface. See
> `../hygg-pwa/README.md`.

The hygg reader as a **native [iced] desktop GUI** for **macOS, Linux, and
Windows** — a real app you can set as your **default PDF / EPUB / text reader**.

It reuses the exact hygg pipeline the CLI and PWA use (`cli-justify`,
`cli-pdf-to-text`, `cli-epub-to-text`), so a document is the same justified
monospace column everywhere — the same "hygg look". `hygg-pwa` is the UX
reference (and the **browser** reader); this crate mirrors its Home dashboard,
Reader, and Settings, rendered natively.

## Why another reader crate

`hygg-pwa` (Leptos + DOM) is the offline touch experience in a browser.
`hygg-gui` takes that same experience **native**, so hygg can be the system's
default document application and run without a browser at all.

## Build & run

```sh
cargo run -p hygg-gui
cargo run -p hygg-gui -- ~/Documents/book.pdf   # open a document directly
```

The crate is kept out of the workspace `default-members` (it pulls the heavy
iced/wgpu stack), so a bare `cargo build` skips it. Build it explicitly with
`-p hygg-gui`.

## Layout

| Path | What |
| --- | --- |
| `src/app/` | iced application: state, messages, async tasks, entry points |
| `src/screens/` | `home` (library dashboard), `reader` (virtualized column), `settings` |
| `src/select.rs` | reader text-selection geometry (monospace → `(line, column)`) |
| `src/model.rs` | `Book` / `BookSummary` / `Progress` — shared identity with the CLI/PWA |
| `src/format.rs` | import: bytes → extracted, justified `Book` (reuses the hygg pipeline) |
| `src/storage/` | offline store — JSON files in the per-user data directory |
| `src/theme.rs` | the hygg palette (dark/light/sepia) mapped onto iced |
| `assets/fonts/` | bundled **Fira Mono** (SIL OFL) — the reader's monospace, registered at startup |
| `platform/` | desktop file-association installers (macOS/Linux/Windows) — see `platform/README.md` |

## Offline-first

Everything works with no network: import runs locally (the pipeline is compiled
in) and documents and reading positions live in the on-device store. A sync
server is optional (Settings → Server) and additive.

## Default document reader

`platform/` ships one-shot installers that register hygg as a handler for PDF /
EPUB / text and, optionally, set it as the default. The app opens documents from
`argv` and from files dropped on the window. See `platform/README.md` for the
per-OS steps and the one macOS caveat (Finder double-click of a bundled app).

## License

AGPL-3.0-only, like the TUI and the PWA. The bundled reader font,
`assets/fonts/FiraMono-Medium.ttf`, is **Fira Mono** by Carrois Corporate GbR &
Edenspiekermann AG, licensed under the SIL Open Font License 1.1
(`assets/fonts/FiraMono-LICENSE`).

[iced]: https://iced.rs
