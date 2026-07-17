### [<-](../README.md)

## Releases
What a release contains, which file you want, and how the version and the
changelog on it were decided. For installing from a package manager or building
from source instead, see [Getting Started](getting-started.md) and
[Detailed Installation](detailed-installation.md).

A release is one git tag. Pushing it fires four workflows at once: the artifacts
below are built and attached, the crates go to crates.io, the server image goes
to GHCR, and the web app deploys to Pages. They finish at different times, so a
release can be a few minutes old before every download beside it exists.

## Which file do I want?
Nothing here is required. The CLI is on crates.io, the web app needs no
download at all, and these archives exist for the case where you would rather
not build or install anything.

| File | Platform | What it is |
|---|---|---|
| `hygg-cli-<tag>-macos-universal.tar.gz` | macOS | The Vim-like TUI. One binary, Apple silicon + Intel |
| `hygg-cli-<tag>-x86_64-linux.tar.gz` | Linux | The TUI |
| `hygg-cli-<tag>-x86_64-windows.zip` | Windows | The TUI |
| `hygg-desktop-<tag>-*.dmg` | macOS | Native desktop app |
| `hygg-desktop-<tag>-*.{deb,AppImage,rpm}` | Linux | Native desktop app |
| `hygg-desktop-<tag>-*.{msi,exe}` | Windows | Native desktop app |
| `hygg-android-<tag>-*.apk` | Android | Sideloadable app |
| `hygg-ios-<tag>-simulator.app.zip` | iOS | Simulator only — see below |
| `SHA256SUMS` | — | Checksums for everything above |

### The CLI archives
Unpack and put `hygg` on your PATH, or skip the download:
```sh
cargo install --locked hygg
```
ODT and DOCX are converted by shelling out to [pandoc](https://pandoc.org),
which is deliberately not bundled — install it separately if you read those.

### The desktop bundles
**These are unsigned.** macOS Gatekeeper and Windows SmartScreen will warn on
first launch, because signing them needs an Apple Developer certificate and a
Windows code-signing certificate that this project does not have. On macOS,
right-click the app and choose Open to get the override dialog.

### The Android APK
Usually `-debug.apk`: debug-signed, because release signing needs secrets that
are not configured on every build. It sideloads and runs fine, but **the signing
key is regenerated per build**, so Android will refuse to install it over a
previous version — uninstall the old one first rather than upgrading in place.
When the signing secrets are present the artifact is `-release.apk` instead and
upgrades normally.

### The iOS build
A simulator build. It **cannot be installed on an iPhone** — that needs a signed
`.ipa` and an Apple Developer certificate. It is here to prove the iOS target
builds, and it is useful to you only if you run Xcode's simulator.

### Verifying a download
```sh
sha256sum -c SHA256SUMS --ignore-missing
```
These checksums say the file is the one the runner built and uploaded. They are
not signatures: they are published on the same release as the files, so they
prove integrity, not provenance.

## The other half of a release
Not everything ships as a file on the release page.

**Web app** — no download, nothing to install. Runs offline once loaded and
installs to a home screen from the browser's own menu.
- latest: https://kruseio.github.io/hygg/
- pinned to one version: `https://kruseio.github.io/hygg/<tag>/`
- every version: https://kruseio.github.io/hygg/versions.html

**Crates** — `cargo install --locked hygg`, or depend on any workspace member
from crates.io. Published versions are immutable; a bad one is yanked, never
replaced.

**Server** — the sync server is optional and self-hosted. linux/amd64 and
linux/arm64:
```sh
docker run -d -p 3032:3032 -v "$PWD/hygg-data:/app/data" ghcr.io/kruseio/hygg-server:<tag>
```

## A missing platform
Every build leg uploads independently and the release is created from whatever
arrived, so one red platform costs that platform's file and nothing else. When
that happens the notes carry a warning naming the gap.

**The asset list on the release is the truth.** The warning names a leg, and a
leg covers several platforms at once — "desktop" is red if any one of macOS,
Linux, or Windows failed, while the other two are attached and fine.

## Where the version and the changelog come from
Neither is written by hand. Both are read out of the commits, by
[git-cliff](https://github.com/orhun/git-cliff), through
[`tools/cliff.toml`](../../tools/cliff.toml).

Every commit follows [Conventional Commits](https://www.conventionalcommits.org)
— `tools/hooks/commit-msg` rejects the ones that don't — which makes the type on
each commit an input to the release rather than a label on it:

| Commits since the last tag | Next version |
|---|---|
| `fix:`, `feat:`, `docs:`, `ci:`, `chore:` … | patch — `0.1.25` → `0.1.26` |
| `feat!:`, or a `BREAKING CHANGE:` footer | minor — `0.1.25` → `0.2.0` |

While the major is 0, `feat:` takes a patch rather than a minor. That is the
semver spec's own rule for `0.y.z`, which it defines as initial development
where anything may change at any time — and it is what this project's releases
were already doing by hand.

The same config means the strict thing the moment the major reaches 1, with
nothing to change: fix → patch, feat → minor, breaking → major. So `1.0.0` is a
decision someone makes by passing it to `tools/.tag.sh` explicitly, and not
something a single `feat!:` can tip the project into on its behalf.

The changelog is generated once, at tag time, and used twice: it is the tag's
annotation — visible on the [tags page](https://github.com/kruseio/hygg/tags)
and in `git show <tag>` — and it is the body of the GitHub release. They are the
same text because the release reads it back off the tag, so the two cannot
drift. Commits of every type appear in it under their own heading; the type only
decides how far the version moves.

See [Development](development.md) for cutting one.
