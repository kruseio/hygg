# Desktop integration — hygg as your default document reader

hygg-gui reads the document path from `argv[1]` and from files **dropped onto its
window**, so once the OS is told hygg handles a file type, opening a document
launches straight into the reader. Each platform has a one-shot installer here.

| Platform | Install | Set as default |
| --- | --- | --- |
| macOS | `platform/macos/bundle.sh` → builds `hygg-gui.app` | Finder → Get Info → *Open with* → **hygg** → *Change All…* |
| Linux (GNOME/XDG) | `platform/linux/install.sh` | add `--default`, or Files → *Open With* |
| Windows | `platform/windows/install.ps1` | add `-Default`, or *Open with → Choose another app → Always* |

## "About hygg" — version & provenance

Every build knows which commit it came from: `build.rs` bakes the git sha + date
into the binary, and the in-app **Settings → About** screen shows the version,
commit hash (linking to it on GitHub), author, and repository. A **Credits**
screen (Settings → About → Credits, or the Credits button) pulls the author and
all repository contributors from GitHub with their avatars, and hosts the
"support the project" button.

Each OS also exposes the build metadata through its own native surface, so the
version/commit is visible even before launching:

- **macOS** — `bundle.sh` substitutes the live version into `Info.plist` and
  writes a `Resources/Credits.html`, so the standard system **About hygg** panel
  shows the version, commit, author, and GitHub link (no native menu code).
- **Windows** — `build.rs` embeds a `VERSIONINFO` resource into `hygg-gui.exe`
  (via the `winresource` build dep, Windows host only), so **right-click the exe
  → Properties → Details** shows the version, publisher (kruseio), copyright,
  and the commit (in *Comments*). A missing resource compiler is a warning, not
  a build failure.
- **Linux (GNOME/KDE)** — `install.sh` installs an AppStream
  `com.kruseio.hygg-gui.metainfo.xml` (with the version + commit date
  substituted) to `~/.local/share/metainfo/`, so **GNOME Software / KDE
  Discover** show hygg's version, description, developer, and links on its
  detail page.

The in-app **Settings → About** screen is the canonical, always-available
surface on every platform.

## How the file reaches the app

- **argv** — Linux (`Exec=hygg-gui %f`), Windows (`"hygg-gui.exe" "%1"`), and any
  shell (`hygg-gui book.pdf`) pass the path directly. Picked up in
  `app::launch()`.
- **drag-and-drop** — dropping a file on the window emits winit's
  `FileDropped` event, handled everywhere (`Message::FileOpened`).

## macOS double-click — known limitation

When a **bundled** `.app` is opened by double-clicking a document in Finder,
macOS delivers the path through an Apple Event (`kAEOpenDocuments`), **not**
`argv`. iced/winit does not surface that event yet, so in this first cut a Finder
double-click focuses hygg but does not auto-open the file. What works today on
macOS:

- drag the document onto the hygg window, and
- `open -a hygg-gui.app --args /path/to/book.pdf` (or run the binary directly).

The follow-up is a small `objc2` shim that installs an `NSApplicationDelegate`
`application:openURLs:` handler and forwards the URLs into the iced runtime as
`Message::FileOpened`. It is isolated to `#[cfg(target_os = "macos")]` and does
not affect the other targets. Linux and Windows double-click work now via argv.
