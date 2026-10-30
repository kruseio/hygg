//! Only the EPUB container, manifest and spine are needed for text extraction.
//! Keep ZIP handling on a patched release, with matching local/central headers.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use percent_encoding::percent_decode_str;
use roxmltree::Document;
use zip::ZipArchive;
use zip::read::ZipReadOptions;

use crate::EpubError;

pub(crate) struct EpubDoc<R: Read + Seek> {
  archive: ZipArchive<R>,
  pub(crate) spine: Vec<String>,
  resources: HashMap<String, String>,
}

fn invalid(error: impl std::fmt::Display) -> EpubError {
  EpubError::InvalidEpub(error.to_string())
}

impl EpubDoc<BufReader<File>> {
  pub(crate) fn new(path: &Path) -> Result<Self, EpubError> {
    Self::from_reader(BufReader::new(File::open(path).map_err(invalid)?))
  }
}

impl<R: Read + Seek> EpubDoc<R> {
  pub(crate) fn from_reader(reader: R) -> Result<Self, EpubError> {
    let mut book = Self {
      archive: ZipArchive::new(reader).map_err(invalid)?,
      spine: Vec::new(),
      resources: HashMap::new(),
    };
    let container = book.required_entry("META-INF/container.xml")?;
    let container = hygg_shared::text_decode::decode_markup(&container).text;
    let container = Document::parse(&container).map_err(invalid)?;
    let rootfile = container
      .descendants()
      .find(|node| {
        node.has_tag_name("rootfile")
          && node.attribute("media-type")
            == Some("application/oebps-package+xml")
      })
      .and_then(|node| node.attribute("full-path"))
      .ok_or_else(|| invalid("Missing EPUB package rootfile"))?;
    let rootfile = resolve("", rootfile)?;
    let package = book.required_entry(&rootfile)?;
    let package = hygg_shared::text_decode::decode_markup(&package).text;
    let package = Document::parse(&package).map_err(invalid)?;
    let root = package.root_element();
    if !root.has_tag_name("package") {
      return Err(invalid("Missing EPUB package element"));
    }
    let base = rootfile.rsplit_once('/').map_or("", |(base, _)| base);
    let manifest = root
      .children()
      .find(|node| node.has_tag_name("manifest"))
      .ok_or_else(|| invalid("Missing EPUB manifest"))?;
    for item in manifest.children().filter(|node| node.has_tag_name("item")) {
      if let (Some(id), Some(href)) =
        (item.attribute("id"), item.attribute("href"))
      {
        book.resources.insert(id.to_owned(), href.to_owned());
      }
    }
    let spine = root
      .children()
      .find(|node| node.has_tag_name("spine"))
      .ok_or_else(|| invalid("Missing EPUB spine"))?;
    for item in spine.children().filter(|node| node.has_tag_name("itemref")) {
      if let Some(id) = item.attribute("idref") {
        book.spine.push(id.to_owned());
      }
    }
    // Resolve only spine resources: other manifest entries can be remote media.
    let mut resources = HashMap::new();
    for id in &book.spine {
      if let Some(href) = book.resources.get(id) {
        resources.insert(id.clone(), resolve(base, href)?);
      }
    }
    book.resources = resources;
    Ok(book)
  }

  pub(crate) fn get_resource(
    &mut self,
    id: &str,
  ) -> Result<Option<Vec<u8>>, EpubError> {
    match self.resources.get(id).cloned() {
      Some(path) => self.entry(&path),
      None => Ok(None),
    }
  }

  fn required_entry(&mut self, path: &str) -> Result<Vec<u8>, EpubError> {
    self
      .entry(path)?
      .ok_or_else(|| invalid(format!("Missing EPUB entry: {path}")))
  }

  fn entry(&mut self, path: &str) -> Result<Option<Vec<u8>>, EpubError> {
    // Match literal names first, as the previous reader did, then URL decoding.
    let decoded = percent_decode_str(path).decode_utf8().map_err(invalid)?;
    let index = self
      .archive
      .index_for_name(path)
      .or_else(|| self.archive.index_for_name(&decoded));
    let Some(index) = index else {
      return Ok(None);
    };
    let mut entry = self
      .archive
      .by_index_with_options(
        index,
        ZipReadOptions::new().enforce_header_consistency(true),
      )
      .map_err(invalid)?;
    if entry.is_symlink() {
      return Err(invalid("EPUB resources must not be symbolic links"));
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(invalid)?;
    Ok(Some(bytes))
  }
}

// EPUB paths are URL paths inside the archive, independent of the host OS.
fn resolve(base: &str, href: &str) -> Result<String, EpubError> {
  let href = href.split(['#', '?']).next().unwrap_or(href);
  if href.starts_with('/') || href.contains(':') || href.contains('\\') {
    return Err(invalid("EPUB resource must be an archive-relative path"));
  }
  let mut parts: Vec<&str> =
    base.split('/').filter(|p| !p.is_empty()).collect();
  for part in href.split('/') {
    match part {
      "" | "." => {}
      ".." => {
        parts.pop().ok_or_else(|| invalid("EPUB path escapes archive root"))?;
      }
      _ => parts.push(part),
    }
  }
  Ok(parts.join("/"))
}
