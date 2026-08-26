### [<-](../README.md)

## Getting Started
hygg reads in the terminal, in a browser, on the desktop, and on your phone.
Every client reads the same documents — pick one and go.

### Install the Rust toolchain
For UNIX type operating systems run the following command:
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

For further install instructions, read the Rust docs https://www.rust-lang.org/learn/get-started

### Install and run with cargo
```sh
cargo install --locked hygg
hygg doc.pdf
```

If the `hygg` binary executable is not found, checkout how to add `~/.cargo/bin` to your path.

e.g. for the fish shell you add the following to your config:

~/.config/fish/config.fish
```fish
fish_add_path ~/.cargo/bin
```

### Advanced install options
Install a specific version
```sh
cargo install --locked --version 0.1.26 hygg
```

Install a specific git branch
```sh
cargo install --locked --git https://github.com/kruseio/hygg --branch cross-platform-which hygg
```

### Download a prebuilt binary
No toolchain needed. Grab the archive for your platform from the
[latest release](https://github.com/kruseio/hygg/releases/latest), unpack it and
put `hygg` on your path. The macOS build is a universal binary.

### Read in the browser
Nothing to install:
```
https://kruseio.github.io/hygg/
```

Add it to your home screen and it reads offline like a native app.

### Install the desktop app
Download your platform's installer from the
[latest release](https://github.com/kruseio/hygg/releases/latest):
`.dmg` (macOS), `.deb` / `.AppImage` / `.rpm` (Linux), `.msi` / `.exe` (Windows).

The installers are unsigned, so macOS Gatekeeper and Windows SmartScreen warn on
first launch.

### Install on mobile
Android — sideload the `.apk` from the
[latest release](https://github.com/kruseio/hygg/releases/latest). It is
debug-signed, so uninstall any previous version rather than upgrading in place.

iOS — no installable build yet; the released iOS app runs in the Xcode simulator
only.

### Run the sync server
Optional. hygg reads fine with no server, and never depends on one. To sync
documents and reading progress across devices, run your own:
```sh
docker run -d -p 3032:3032 -v "$PWD/hygg-data:/app/data" ghcr.io/kruseio/hygg-server:latest
```

Then point a client at `http://localhost:3032`. The server keeps its database
and logs in the `hygg-data` directory this creates, and will refuse to start if
you point that mount at a directory holding anything else.

## Additional formats via pandoc
```sh
sudo apt install pandoc
# scoop install pandoc
# brew install pandoc
hygg doc.docx
```

## OCR for scanned documents
Install with the English OCR feature to enable OCR for scanned PDFs:
```sh
cargo install --locked --features ocr hygg
hygg --ocr=on doc.pdf
```

When installing from a local checkout, pass the same feature flag to the
`hygg` package:
```sh
cargo install --locked --path hygg --features ocr
hygg --ocr=on doc.pdf
```

The models are not bundled: on first use they download (~10 MB) from the
project's `ocr-models-v1.0` release, are verified against a pinned checksum, and
cached under your platform cache dir (`HYGG_OCR_MODEL_DIR` overrides the location
for offline use). An `--features ocr` build has OCR on by default; toggle it per
run with `--ocr=off` or the `HYGG_OCR` environment variable.

The bundled OCR feature does not require `ocrmypdf` or Tesseract.

## Reading from stdin
```sh
cat README.md | hygg
curl example.com | hygg
pandoc doc.docx --to=plain | hygg
```

## Going further
[Detailed installation](detailed-installation.md) covers the rest: building any
client from source, text-to-speech, per-platform prerequisites, and self-hosting
the server and the web app.
