### [<-](../README.md)

## Getting Started
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
cargo install --locked --version 0.1.18 hygg
```

Insall a specific git branch
```sh
cargo install --locked --git https://github.com/kruseio/hygg --branch cross-platform-which hygg
```

Clone the repo, build from source and run
```sh
git clone https://github.com/kruseio/hygg.git
cd hygg
cargo run -- test-data/pdf/pdfreference1.7old-1-50.pdf
```

Clone the repo, build from source, install and run
```sh
git clone https://github.com/kruseio/hygg.git
cd hygg
cargo install --locked --path hygg
hygg test-data/pdf/pdfreference1.7old-1-50.pdf
```

## Additional formats via pandoc
```sh
sudo apt install pandoc
# scoop install pandoc
# brew install pandoc
hygg doc.docx
```

## OCR for scanned documents
```sh
sudo apt install ocrmypdf tesseract-ocr-eng
hygg --ocr=true doc.pdf
```

## Reading from stdin
```sh
cat README.md | hygg
curl example.com | hygg
pandoc doc.docx --to=plain | hygg
```
