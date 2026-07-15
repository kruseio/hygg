pub(crate) fn append_pdf_paragraph_fragment(
  paragraph: &mut String,
  line: &str,
) {
  let fragment = line.trim();
  if fragment.is_empty() {
    return;
  }

  if paragraph.is_empty() {
    paragraph.push_str(fragment);
    return;
  }

  if paragraph.ends_with('-')
    && fragment.chars().next().is_some_and(|ch| ch.is_alphabetic())
  {
    let is_compound = word_before_hyphen_is_compound_prefix(paragraph);
    if is_compound {
      // Hard hyphen between two complete words — keep the hyphen and join
      // the fragment without an extra space.
      paragraph.push_str(fragment);
    } else {
      // Soft hyphen from PDF line wrapping — drop the hyphen and join.
      paragraph.pop();
      paragraph.push_str(fragment);
    }
    return;
  }

  paragraph.push(' ');
  paragraph.push_str(fragment);
}

fn word_before_hyphen_is_compound_prefix(paragraph: &str) -> bool {
  let without_hyphen = paragraph.trim_end_matches('-');
  let last_word = without_hyphen
    .rsplit(|ch: char| ch.is_whitespace())
    .next()
    .unwrap_or_default();
  if last_word.is_empty() {
    return false;
  }

  let lower = last_word.to_ascii_lowercase();
  matches!(
    lower.as_str(),
    "cross"
      | "multi"
      | "semi"
      | "anti"
      | "non"
      | "self"
      | "post"
      | "pre"
      | "sub"
      | "super"
      | "ultra"
      | "inter"
      | "intra"
      | "trans"
      | "mid"
      | "mini"
      | "micro"
      | "macro"
      | "mega"
      | "hyper"
      | "hypo"
      | "over"
      | "under"
      | "well"
      | "ill"
      | "low"
      | "high"
      | "full"
      | "half"
      | "all"
      | "co"
      | "ex"
      | "re"
      | "de"
      | "un"
      | "in"
      | "on"
      | "off"
      | "long"
      | "short"
      | "deep"
      | "left"
      | "right"
      | "two"
      | "one"
      | "open"
      | "free"
      | "real"
      | "user"
      | "page"
      | "line"
      | "file"
      | "code"
      | "data"
      | "web"
      | "side"
      | "front"
      | "back"
      | "top"
      | "end"
      | "form"
      | "name"
      | "node"
      | "type"
      | "case"
      | "word"
      | "size"
      | "time"
      | "year"
      | "site"
      | "view"
      | "test"
      | "team"
      | "tag"
      | "stream"
      | "object"
      | "color"
      | "device"
      | "system"
      | "platform"
      | "network"
      | "language"
      | "encoding"
      | "letter"
      | "english"
      | "latin"
      | "white"
      | "black"
      | "mac"
      | "unix"
      | "x"
      | "y"
      | "z"
      | "m"
      | "n"
      | "t"
  )
}
