<p align="center">
  <a href="https://github.com/kruseio/hygg" target="_blank">
    <img width="300" src="https://raw.githubusercontent.com/kruseio/hygg/main/assets/logo/logo.svg">
  </a>
</p>

# hygg
📚 Simplifying the way you read.
<br>
Minimalistic Vim-like TUI document reader.

<br>
<br>

<p align="center">
  <img src="https://raw.githubusercontent.com/kruseio/hygg/main/assets/demo-1.gif" alt="animated" />
</p>

<br>

## Why hygg?

- **Universal document support** - PDF, EPUB, DOCX, and many more formats via pandoc. Supports scanned documents with OCR
- **Lightning-fast keyboard based navigation** - Vim-inspired keybindings
- **Powerful search** - Find anything instantly, highlight important passages, add bookmarks
- **Never lose your place** - Automatic progress saving
- **Extensible workflows** - Execute commands directly from copied text
- **Respects your privacy** - Run locally without server, or selfhost the sync server

## Quick start guide
```sh
cargo install --locked hygg
hygg doc.pdf
```

If you are a wayland user(it requires specific clipboard protocols):
```sh
cargo install --locked --features wayland hygg
hygg doc.pdf
```

For further install instructions read the [Getting started page](https://github.com/kruseio/hygg/blob/main/docs/pages/getting-started.md)

## Features

### Core Reading Experience
- **Minimalist interface** - Nothing between you and your content
- **Smart text justification** - Perfectly formatted for your terminal width
- **Vim keybindings** - Navigate with the efficiency you already know
- **Visual selection** - Select, copy, highlight text
- **Bookmarks** - Set bookmark points and jump between them instantly
- **Persistent** - Progress and bookmarks persist

### Advanced Workflows
- **Command execution** - Run shell commands from copied text, also works for previous command output
- **Split view** - View command output alongside your document
- **Interactive tutorial** - Learn everything in under 5 minutes
- **Cross-platform** - Works on Linux, macOS, and Windows

## Roadmap
- [x] Plain text format support
- [x] PDF format support
- [x] EPUB format support
- [x] Convert scanned documents and images to plain text with ocrmypdf
- [x] Auto saving progress
- [x] Integrated command line with vim like commands
- [x] Text selection and yanking
- [x] Execute commands from yanked text
- [x] Text highlighting
- [x] Bookmarks
- [x] Interactive tutorial
- [ ] Self hosted sync server for docs, progress, bookmarks, highlights and notes
- [ ] Offline PWA web client
- [ ] Start screen to show overview of books and progress
- [ ] Minimal build feature flag, for only basic converters, no server integration and no ai
- [ ] Image to ascii art converter
- [ ] Natural sounding ai voice model for text to speech narration
- [ ] Run all inference directly in rust no external runtime deps
- [ ] Support more ebook and document formats
- [ ] AI-powered document summarization
- [ ] Reading statistics and insights

## Documentation
Visit the [Documentation](https://github.com/kruseio/hygg/blob/main/docs/README.md)

## Community

**hygg** is built by readers, for readers. Join our growing community:

- [Report issues](https://github.com/kruseio/hygg/issues)
- [Request features](https://github.com/kruseio/hygg/issues)
- [Contribute code](https://github.com/kruseio/hygg/pulls)

## Star History

<a href="https://www.star-history.com/#kruseio/hygg&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kruseio/hygg&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kruseio/hygg&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kruseio/hygg&type=Date" />
 </picture>
</a>

## Contributors
<a href="https://github.com/kruseio/hygg/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=kruseio/hygg" />
</a>

Made with [contrib.rocks](https://contrib.rocks).
