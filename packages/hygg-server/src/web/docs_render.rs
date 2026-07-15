//! Markdown -> HTML rendering for the docs center: parse a page, give every
//! heading a unique slug id, and collect its table of contents and searchable
//! sections in a single event walk.

use std::collections::{HashMap, HashSet};

use pulldown_cmark::{
  CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html,
};

use super::*;

/// Render one page: parse the markdown, assign a unique slug to every heading,
/// collect the table of contents and searchable sections, then emit HTML with
/// those slugs as `id`s so links can jump straight to a heading.
pub(crate) fn build_doc(src: &DocSource) -> RenderedDoc {
  let cleaned = strip_back_nav(src.markdown);
  let mut opts = Options::empty();
  opts.insert(Options::ENABLE_TABLES);
  opts.insert(Options::ENABLE_STRIKETHROUGH);
  let events: Vec<Event> = Parser::new_ext(&cleaned, opts).collect();

  // Pass 1: a unique slug (+ text + level) for each heading, keyed by the
  // heading's start-event index so passes 2 and 3 can look it up.
  let mut used: HashSet<String> = HashSet::new();
  let mut headings: HashMap<usize, (String, String, u8)> = HashMap::new();
  for (idx, ev) in events.iter().enumerate() {
    if let Event::Start(Tag::Heading { level, .. }) = ev {
      let title = heading_text(&events, idx);
      let slug = unique_slug(&slugify(&title), &mut used);
      headings.insert(idx, (slug, title, heading_num(*level)));
    }
  }

  // Pass 2: table of contents + searchable sections. Each heading opens a new
  // section; text before the first heading (if any) stays in the lead section.
  let mut toc: Vec<TocItem> = Vec::new();
  let mut sections: Vec<DocSection> = Vec::new();
  let mut current = DocSection {
    slug: String::new(),
    title: src.title.to_string(),
    text: String::new(),
  };
  // The heading's own text is captured as the section title, so skip it in the
  // body; a separator is inserted at block boundaries so words from adjacent
  // blocks don't run together in snippets ("feature.Install" -> "feature.
  // Install").
  let mut in_heading = false;
  for (idx, ev) in events.iter().enumerate() {
    match ev {
      Event::Start(Tag::Heading { .. }) => {
        in_heading = true;
        if let Some((slug, title, level)) = headings.get(&idx) {
          let finished = std::mem::replace(
            &mut current,
            DocSection {
              slug: slug.clone(),
              title: title.clone(),
              text: String::new(),
            },
          );
          if !finished.text.trim().is_empty() {
            sections.push(finished);
          }
          toc.push(TocItem {
            slug: slug.clone(),
            title: title.clone(),
            level: *level,
          });
        }
      }
      Event::End(TagEnd::Heading(_)) => in_heading = false,
      Event::Text(t) | Event::Code(t) if !in_heading => {
        current.text.push_str(t)
      }
      Event::SoftBreak | Event::HardBreak if !in_heading => {
        push_separator(&mut current.text)
      }
      Event::End(
        TagEnd::Paragraph
        | TagEnd::Item
        | TagEnd::CodeBlock
        | TagEnd::TableCell
        | TagEnd::TableRow,
      ) => push_separator(&mut current.text),
      _ => {}
    }
  }
  if !current.text.trim().is_empty() {
    sections.push(current);
  }

  // Pass 3: render to HTML, injecting each heading's slug as its `id`.
  let mut render_events: Vec<Event> = Vec::with_capacity(events.len());
  for (idx, ev) in events.into_iter().enumerate() {
    match ev {
      Event::Start(Tag::Heading { level, classes, attrs, .. }) => {
        let id =
          headings.get(&idx).map(|(slug, _, _)| CowStr::from(slug.clone()));
        render_events.push(Event::Start(Tag::Heading {
          level,
          id,
          classes,
          attrs,
        }));
      }
      other => render_events.push(other),
    }
  }
  let mut html = String::new();
  html::push_html(&mut html, render_events.into_iter());

  RenderedDoc { slug: src.slug, title: src.title, html, toc, sections }
}

/// Drop the `[<-](../README.md)` back-navigation line the repo docs carry for
/// GitHub browsing; it is meaningless (and a broken link) inside the web app.
fn strip_back_nav(md: &str) -> String {
  md.lines()
    .filter(|line| !line.contains("[<-]"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The plain text of the heading that starts at `start`, concatenating its text
/// and inline-code runs up to the closing tag.
fn heading_text(events: &[Event], start: usize) -> String {
  let mut text = String::new();
  let mut i = start + 1;
  while i < events.len() {
    match &events[i] {
      Event::End(TagEnd::Heading(_)) => break,
      Event::Text(t) | Event::Code(t) => text.push_str(t),
      _ => {}
    }
    i += 1;
  }
  text.trim().to_string()
}

/// Append a single space to separate the text of adjacent blocks, unless the
/// buffer is empty or already ends in a space.
fn push_separator(text: &mut String) {
  if !text.is_empty() && !text.ends_with(' ') {
    text.push(' ');
  }
}

fn heading_num(level: HeadingLevel) -> u8 {
  match level {
    HeadingLevel::H1 => 1,
    HeadingLevel::H2 => 2,
    HeadingLevel::H3 => 3,
    HeadingLevel::H4 => 4,
    HeadingLevel::H5 => 5,
    HeadingLevel::H6 => 6,
  }
}

/// A URL-safe slug: lowercase ASCII alphanumerics, other runs collapsed to a
/// single `-`. Non-ASCII (emoji, accents) is dropped, which is fine for
/// anchors.
fn slugify(text: &str) -> String {
  let mut slug = String::new();
  for ch in text.chars() {
    if ch.is_ascii_alphanumeric() {
      slug.push(ch.to_ascii_lowercase());
    } else if matches!(ch, ' ' | '-' | '_' | '.' | '/')
      && !slug.is_empty()
      && !slug.ends_with('-')
    {
      slug.push('-');
    }
  }
  while slug.ends_with('-') {
    slug.pop();
  }
  if slug.is_empty() { "section".to_string() } else { slug }
}

/// Ensure a slug is unique within a page by appending `-2`, `-3`, … on
/// collision.
fn unique_slug(base: &str, used: &mut HashSet<String>) -> String {
  if used.insert(base.to_string()) {
    return base.to_string();
  }
  let mut n = 2;
  loop {
    let candidate = format!("{base}-{n}");
    if used.insert(candidate.clone()) {
      return candidate;
    }
    n += 1;
  }
}
