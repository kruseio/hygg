# hygg-tauri

The **native app shell** for hygg — a [Tauri v2](https://tauri.app) wrapper that
renders the **exact `hygg-pwa` Leptos UI** and ships it as a real installable app
to **desktop (Windows, macOS, Linux/GNOME)** and **mobile (iOS, Android)** from
one frontend.

> **Status: activated.** A workspace member (kept out of `default-members`).
> The frontend IPC dispatch, native extraction commands, Tauri guards, macOS
> desktop bundle, iOS Xcode project, and CI leg are in place. Build commands are
> under "Build" below.

## The idea

One UX, every platform. `hygg-pwa` is the canonical reader; the browser serves it
as a PWA, and this crate wraps that same `dist/` bundle in an OS webview. The
**one change on native**: the heavy document pipeline (`cli-justify`,
`cli-epub-to-text`, `cli-pdf-to-text`) runs as a **native Tauri IPC command**
([`extract_document`] in `src/lib.rs`) instead of in wasm — native-speed
extraction, no multi-MB wasm cold-compile tax (which hurts most on mobile CPUs).
Storage (IndexedDB), sync, and TTS stay in the webview, unchanged.

## Layout

| Path | What |
| --- | --- |
| `src/lib.rs` | `run()` (desktop + mobile entry point) and the `extract_document` IPC command |
| `src/main.rs` | desktop binary → `hygg_tauri_lib::run()` |
| `build.rs` | `tauri_build::build()` |
| `tauri.conf.json` | `frontendDist = ../hygg-pwa/dist`; `beforeBuildCommand` runs `trunk build` |
| `capabilities/default.json` | core permissions only (the webview file input yields the bytes) |

## Build

Prerequisites: a Rust toolchain, [Trunk](https://trunkrs.dev), and the Tauri CLI
(`cargo install --locked tauri-cli`). Run from this directory. `cargo tauri
build` / `dev` first run the `beforeBuildCommand` (a `trunk build --release` of
`hygg-pwa`, resolved via `git rev-parse --show-toplevel` so it works from any
CWD), then embed that `dist/` bundle.

```sh
cargo tauri dev                     # desktop dev — renders dist/ in a webview
cargo tauri build                   # desktop app + installer
```

Desktop macOS is built and verified (the `.app` renders the reader and imports
via native IPC; `cargo test -p hygg-tauri` covers the extraction commands). The
DMG's default `bundle_dmg.sh` step needs a GUI Finder session; in a headless
environment build a DMG from the `.app` with
`hdiutil create -volname hygg -srcfolder <app> -format UDZO out.dmg`.

Mobile — iOS needs macOS + Xcode + CocoaPods (and an Apple Developer team to
sign); Android needs a JDK + the Android SDK/NDK. The iOS Xcode project is
generated under `gen/apple`. Regenerate / build with:

```sh
cargo tauri ios init      # generates gen/apple (Xcode project + Podfile)
cargo tauri ios build     # needs a signing team (set in gen/apple or --ci)
cargo tauri android init  # generates gen/android (Gradle project)
cargo tauri android build # emits an APK/AAB
```

The generated `gen/apple` and `gen/android` projects are git-ignored until you
customize signing/manifests, then commit them deliberately (see `.gitignore`).

## Isolation

`publish = false`, and nothing in the published `hygg` crate's dependency tree
references this crate — so `cargo install hygg` never pulls Tauri. Same posture
as `hygg-pwa` and `hygg-server`.

## License

AGPL-3.0-only, like the rest of the hygg reader clients.
