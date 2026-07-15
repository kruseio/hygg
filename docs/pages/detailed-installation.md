### [<-](../README.md)

## Detailed installation
Everything past the one-shot installs in [Getting Started](getting-started.md):
building each client from source, the optional features, per-platform
prerequisites, and self-hosting.

Every client reads the same documents and identifies them the same way, so a
book imported on one shows up as the same book on another once sync is on.

| Client | Crate | What it is | Ways in |
|---|---|---|---|
| Terminal reader | `hygg` | The Vim-like TUI — the flagship | crates.io, prebuilt archive, source |
| Web app | `hygg-pwa` | Offline touch reader, installs to the home screen | hosted, self-hosted, source |
| Desktop app | `hygg-tauri` | Native shell around the web app's UI | installer, source |
| Mobile app | `hygg-tauri` | The same UI on iOS and Android | APK, source |
| Native desktop GUI | `hygg-gui` | iced, no webview — **paused / feature-frozen** | source only |
| Sync server | `hygg-server` | Optional, self-hostable | container image, source |

## Terminal reader (CLI)

### From crates.io
```sh
cargo install --locked hygg
hygg doc.pdf
```

### A specific version
```sh
cargo install --locked --version 0.1.21 hygg
```

### A specific git branch
```sh
cargo install --locked --git https://github.com/kruseio/hygg --branch cross-platform-which hygg
```

### From a local checkout
Build and run without installing:
```sh
git clone https://github.com/kruseio/hygg.git
cd hygg
cargo run -- test-data/pdf/pdfreference1.7old-1-50.pdf
```

Or install the checkout onto your path:
```sh
git clone https://github.com/kruseio/hygg.git
cd hygg
cargo install --locked --path hygg
hygg test-data/pdf/pdfreference1.7old-1-50.pdf
```

### Prebuilt binaries
Every release attaches an archive per platform —
`hygg-cli-<tag>-x86_64-linux.tar.gz`, `hygg-cli-<tag>-macos-universal.tar.gz`
(Apple silicon + Intel in one binary), and `hygg-cli-<tag>-x86_64-windows.zip`.
Unpack, put `hygg` on your path, done. See [verifying a
download](#verifying-a-download).

### Optional: OCR for scanned documents
Install with the bundled English OCR feature to enable OCR for scanned PDFs:
```sh
cargo install --locked --features pdf-ocr-bundled hygg
hygg --ocr=on doc.pdf
```

When installing from a local checkout, pass the same feature flag to the `hygg`
package:
```sh
cargo install --locked --path hygg --features pdf-ocr-bundled
hygg --ocr=on doc.pdf
```

The bundled OCR feature does not require `ocrmypdf` or Tesseract — the models
ship inside the crate.

### Optional: text to speech
Narrate a document with a local neural voice:
```sh
cargo install --locked --features tts hygg
```

Then run `:speak` inside the reader. The feature needs a C toolchain and CMake at
build time (it builds espeak-ng from vendored sources), and the voice model
downloads on first use rather than shipping with the crate. Voices, speed and the
rest are covered in [Text to Speech](tts.md); note that a redistributed `tts`
build carries extra license obligations, spelled out in
[LICENSING.md](../../LICENSING.md).

## Web app (PWA)

### Hosted
Nothing to install — the browser is the client:

| | |
|---|---|
| Latest | https://kruseio.github.io/hygg/ |
| Pinned to a version | https://kruseio.github.io/hygg/v0.1.21/ |
| Every version | https://kruseio.github.io/hygg/versions.html |

A pinned link keeps working after later releases, which is the point of it.
After the first load the app shell is cached by a service worker, so reading
works with no network at all.

### Build from source
Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target, and
[Trunk](https://trunkrs.dev):
```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

```sh
cargo run -p hygg-pwa                     # dev server + hot reload on http://127.0.0.1:8080
cargo run -p hygg-pwa -- build --release  # production bundle in packages/hygg-pwa/dist
```

`cargo run -p hygg-pwa` is a thin launcher that shells out to Trunk, so you do
not need to `cd` first; everything after `--` is forwarded to Trunk verbatim.
`cd packages/hygg-pwa && trunk serve` works just as well.

### Self-host the web app
`trunk build --release` emits a fully static bundle in `packages/hygg-pwa/dist` — serve it
from any static host. One requirement: an SPA fallback that serves `index.html`
for unknown paths, otherwise deep links like `/read/:id` 404.

Serving from a sub-path (rather than the root of an origin) needs the bundle
built for that path — `trunk build --release --public-url /hygg/v0.1.21/` — plus
`packages/hygg-pwa/tools/prepare_pages_dist.py`, which injects the `<base href>` Trunk
does not and copies `index.html` to `404.html`. This is what
`.github/workflows/pages.yml` does for each tag; `packages/hygg-pwa/README.md` explains
why each path needs its own build.

## Desktop app (Tauri)

The desktop app wraps the same Leptos UI as the web app in an OS webview, and
runs the document pipeline natively instead of in wasm.

### Prebuilt installers
From the [latest release](https://github.com/kruseio/hygg/releases/latest):
`hygg-desktop-<tag>-macos-universal.dmg`, `.deb` / `.AppImage` / `.rpm` for
Linux, `.msi` / `-setup.exe` for Windows. All unsigned — Gatekeeper and
SmartScreen will warn on first launch.

### Build from source
Prerequisites: a Rust toolchain, [Trunk](https://trunkrs.dev), and the Tauri CLI:
```sh
cargo install --locked trunk
cargo install --locked tauri-cli --version "^2"
```

On Linux you also need the WebKitGTK stack Tauri links against, plus AppImage
tooling:
```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
  libayatana-appindicator3-dev patchelf
```

Then, from `packages/hygg-tauri/`:
```sh
cargo tauri dev      # dev build in a webview
cargo tauri build    # app + installer
```

Both first run a `trunk build --release` of `hygg-pwa` and embed the resulting
`dist/` bundle, so there is no separate frontend step.

On macOS, the DMG packaging step needs a GUI Finder session. Headless, build the
`.app` and package it yourself:
```sh
hdiutil create -volname hygg -srcfolder <app> -format UDZO out.dmg
```

## Mobile app (Tauri)

Same UI, same shell, built for a phone. Run these from `packages/hygg-tauri/`.

### Android
Prerequisites: a JDK plus the Android SDK and NDK.
```sh
cargo tauri android init   # generates gen/android (Gradle project)
cargo tauri android build  # emits an APK/AAB
```

The released APK is debug-signed: it sideloads fine, but the signing key differs
per build, so uninstall any previous version instead of upgrading in place.

### iOS
Prerequisites: macOS with Xcode and CocoaPods, and an Apple Developer team to
sign with.
```sh
cargo tauri ios init    # generates gen/apple (Xcode project + Podfile)
cargo tauri ios build   # needs a signing team, set in gen/apple or via --ci
```

Releases ship a simulator build only (`hygg-ios-<tag>-simulator.app.zip`), which
cannot be installed on an iPhone — a device-installable `.ipa` needs an Apple
Developer certificate and provisioning profile.

The generated `gen/apple` and `gen/android` projects are git-ignored until you
customize signing or manifests.

## Native desktop GUI (legacy)

`hygg-gui` is the reader as a native [iced](https://iced.rs) app with no webview.
It is **paused and feature-frozen** — the desktop path forward is the Tauri app
above — but it still builds and runs, and it is the option to reach for if you
want no webview at all.

Source only; there are no prebuilt bundles:
```sh
cargo run -p hygg-gui
cargo run -p hygg-gui -- ~/Documents/book.pdf   # open a document directly
```

It is kept out of the workspace `default-members` because it pulls the heavy
iced/wgpu stack, so a bare `cargo build` skips it — build it explicitly with
`-p hygg-gui`. To register it as the system's default PDF/EPUB/text reader, see
`packages/hygg-gui/platform/README.md`.

## Sync server

Optional in every sense: hygg reads fully offline and never depends on a server.
Run one to sync documents, progress, bookmarks, highlights and notes across
devices. It is a single Rust binary storing everything in SQLite — a file, no
extra services.

### Prebuilt image
Multi-arch (`linux/amd64` + `linux/arm64`, so a Raspberry Pi or Graviton box
works), no toolchain and no compile:
```sh
docker run -d -p 3032:3032 -v "$PWD/data:/app/data" \
  ghcr.io/kruseio/hygg-server:latest
```

Tags are `:latest`, `:0.1.21` (pin this one) and `:0.1`.

### From source
```sh
cd packages/hygg-server
cp .env.example .env    # optional; the defaults work as-is
docker compose up --build -d
```

Set `ADMIN_BOOTSTRAP_EMAIL` / `ADMIN_BOOTSTRAP_PASSWORD` in `.env` to create an
admin account on first boot. Without Docker:
```sh
cd packages/hygg-server
cargo run -p hygg-server   # loads .env from here; data lands in ./data
```

### Check it
```sh
curl http://localhost:3032/health      # {"status":"ok"}
```

Use an explicit `http://` URL. Browsers may auto-upgrade a bare `localhost:3032`
to HTTPS, which this server does not serve — it shows up as
`ERR_CONNECTION_REFUSED`. The web UI is at the same address.

`hygg-server` is source-available under the Elastic License 2.0, not the AGPL
that covers the readers — see [LICENSING.md](../../LICENSING.md). Its own
`packages/hygg-server/README.md` goes deeper on configuration, auth and the API.

## Additional formats via pandoc
PDF, EPUB, TXT and Markdown are handled natively. DOCX, ODT, RTF and the rest go
through [pandoc](https://pandoc.org), which is not bundled:
```sh
sudo apt install pandoc
# scoop install pandoc
# brew install pandoc
hygg doc.docx
```

The web and mobile clients convert those formats on a connected server instead,
since there is no pandoc to shell out to in a browser.

## Verifying a download
Every release includes a `SHA256SUMS` file covering all its artifacts:
```sh
sha256sum -c SHA256SUMS 2>/dev/null | grep -v FAILED
```
