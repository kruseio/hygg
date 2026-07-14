//! Server-rendered documentation center at `/docs`.
//!
//! The repository's `docs/pages/*.md` are embedded verbatim at compile time
//! (`include_str!`, so the site and the repo share one source of truth) and
//! rendered to HTML with a stable slug id on every heading. A full-text search
//! (`/docs/search`) walks every page's sections and links each hit to the exact
//! heading; the target page then highlights and scrolls to the matched text via
//! a tiny inline script (`?q=`), so a result lands the reader on the exact
//! position.
//!
//! This file is the model: the page registry, the rendered types, and the
//! process-wide cache. Rendering lives in [`docs_render`], search in
//! [`docs_search`], and the HTTP handlers in [`docs_view`].
//!
//! [`docs_render`]: super::docs_render
//! [`docs_search`]: super::docs_search
//! [`docs_view`]: super::docs_view

use std::sync::OnceLock;

use super::*;

/// A documentation page: its URL slug, display title, and the raw markdown
/// embedded from the repository's `docs/pages/` directory. Adding a page is a
/// single entry here; the renderer, search index, and index grid pick it up.
pub(crate) struct DocSource {
  pub(crate) slug: &'static str,
  pub(crate) title: &'static str,
  pub(crate) markdown: &'static str,
}

// The paths below go through `hygg-server/docs/pages/`, which holds a symlink
// per page pointing back out at the repository's `docs/pages/`. The indirection
// is what makes this crate publishable: `cargo package` copies only files at or
// under the package root, so the `../../../docs/pages/*.md` these once named
// were simply absent from the tarball and every `cargo publish -p hygg-server`
// died in the verification build — which is why the crate sat at 0.1.15 while
// the rest of the workspace moved on. Cargo resolves each symlink and writes
// the *content* into the tarball as a regular file, so the published crate
// carries real markdown while the repository keeps one copy of it, at the root,
// where README.md and docs/README.md link to it.
//
// Windows caveat: a clone without symlink support (Git's `core.symlinks=false`)
// leaves these as text files holding their target path, which would embed that
// path as a page's body. Enable Developer Mode or `git config core.symlinks
// true` if you build hygg-server there.
const DOC_SOURCES: &[DocSource] = &[
  DocSource {
    slug: "getting-started",
    title: "Getting Started",
    markdown: include_str!("../../docs/pages/getting-started.md"),
  },
  DocSource {
    slug: "text-to-speech",
    title: "Text to Speech",
    markdown: include_str!("../../docs/pages/tts.md"),
  },
  DocSource {
    slug: "reference",
    title: "Reference",
    markdown: include_str!("../../docs/pages/reference.md"),
  },
  DocSource {
    slug: "development",
    title: "Development",
    markdown: include_str!("../../docs/pages/development.md"),
  },
  DocSource {
    slug: "benchmark",
    title: "Benchmark",
    markdown: include_str!("../../docs/pages/benchmark.md"),
  },
];

/// One heading in a page, for the "On this page" table of contents.
pub(crate) struct TocItem {
  pub(crate) slug: String,
  pub(crate) title: String,
  pub(crate) level: u8,
}

/// A searchable slice of a page: everything under one heading. `slug` anchors
/// the section (empty for text before the first heading); `text` is the plain
/// text used for matching and snippets.
pub(crate) struct DocSection {
  pub(crate) slug: String,
  pub(crate) title: String,
  pub(crate) text: String,
}

/// A fully rendered page: HTML with heading anchors, its table of contents, and
/// the flattened sections that back search.
pub(crate) struct RenderedDoc {
  pub(crate) slug: &'static str,
  pub(crate) title: &'static str,
  pub(crate) html: String,
  pub(crate) toc: Vec<TocItem>,
  pub(crate) sections: Vec<DocSection>,
}

/// The rendered docs, built once on first access and cached for the process.
/// Rendering is pure and the inputs are compile-time constants, so this never
/// needs invalidation.
pub(crate) fn docs() -> &'static [RenderedDoc] {
  static CACHE: OnceLock<Vec<RenderedDoc>> = OnceLock::new();
  CACHE.get_or_init(|| DOC_SOURCES.iter().map(build_doc).collect())
}

pub(crate) fn find_doc(slug: &str) -> Option<&'static RenderedDoc> {
  docs().iter().find(|doc| doc.slug == slug)
}
