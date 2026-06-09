pub(crate) fn is_vertical_margin_letter_line(line: &str) -> bool {
  let trimmed_end = line.trim_end();
  if trimmed_end.is_empty() {
    return false;
  }

  let leading_ws =
    trimmed_end.chars().take_while(|ch| ch.is_whitespace()).count();
  if leading_ws < 40 {
    return false;
  }

  let content = trimmed_end.trim_start();
  let mut chars = content.chars();
  let Some(ch) = chars.next() else {
    return false;
  };

  chars.next().is_none() && ch.is_alphabetic()
}

fn has_wide_gap_before_page_number(trimmed: &str) -> bool {
  let Some(last_token) = trimmed.split_whitespace().last() else {
    return false;
  };
  if !last_token.chars().all(|ch| ch.is_ascii_digit()) {
    return false;
  }

  let Some(number_start) = trimmed.rfind(last_token) else {
    return false;
  };
  let before_number = &trimmed[..number_start];
  let gap =
    before_number.chars().rev().take_while(|ch| ch.is_whitespace()).count();

  gap >= 10
}

pub(crate) fn is_running_header_or_footer_line(line: &str) -> bool {
  let trimmed = line.trim();
  if trimmed.is_empty() {
    return false;
  }

  let leading_ws = line.chars().take_while(|ch| ch.is_whitespace()).count();
  let is_page_number_only = trimmed.chars().all(|ch| ch.is_ascii_digit());
  if is_page_number_only && leading_ws >= 20 {
    return true;
  }

  // Per-page running headers in the PDF Reference look like
  //   `CHAPTER 3                                                Syntax`
  //   `SECTION 3.2                                              Objects`
  // — an uppercase label, a section number, a wide gap, then a short
  // title. pdf_oxide preserves them at the top of every page within the
  // chapter, where they otherwise inject a stray heading mid-paragraph
  // once page text is concatenated. The trailing-page branches below
  // can't see them because the last token is a word, not a digit run.
  if is_chapter_section_running_header(trimmed) {
    return true;
  }

  // Front-matter sections ("Figures", "Tables", "Contents") have a
  // single-word centered heading on their first page and a left-
  // aligned single-word running head on each subsequent page. The
  // centered version is preserved as the actual section title via
  // `centered_heading_label`; this branch drops the left-aligned
  // running head so it doesn't wedge itself between adjacent list
  // entries when pages are concatenated.
  if is_left_aligned_section_running_head(line, trimmed) {
    return true;
  }

  let trailing_page = trimmed
    .split_whitespace()
    .last()
    .is_some_and(|token| token.chars().all(|ch| ch.is_ascii_digit()));
  if !trailing_page {
    return false;
  }

  if (trimmed.contains("CHAPTER") || trimmed.contains("SECTION"))
    && leading_ws >= 20
  {
    return true;
  }

  if has_wide_gap_before_page_number(trimmed) {
    let before_number = trimmed
      .split_whitespace()
      .collect::<Vec<_>>()
      .split_last()
      .map(|(_, rest)| rest.join(" "))
      .unwrap_or_default();
    if before_number.split_whitespace().count() <= 6 {
      return true;
    }
  }

  false
}

fn is_left_aligned_section_running_head(line: &str, trimmed: &str) -> bool {
  let leading_ws = line.chars().take_while(|ch| ch.is_whitespace()).count();
  if leading_ws >= 12 {
    return false;
  }
  let mut words = trimmed.split_whitespace();
  let Some(only) = words.next() else {
    return false;
  };
  if words.next().is_some() {
    return false;
  }
  matches!(
    only,
    "Figures"
      | "Tables"
      | "Contents"
      | "Plates"
      | "Bibliography"
      | "Index"
      | "Preface"
      | "Glossary"
  )
}

fn is_chapter_section_running_header(trimmed: &str) -> bool {
  let tokens: Vec<&str> = trimmed.split_whitespace().collect();
  if tokens.len() < 3 || tokens.len() > 6 {
    return false;
  }

  let label = tokens[0];
  if !matches!(label, "CHAPTER" | "SECTION" | "APPENDIX" | "PART") {
    return false;
  }

  let number = tokens[1];
  if number.is_empty() || number.len() > 8 {
    return false;
  }
  if !number.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '.') {
    return false;
  }
  // Accept "3", "3.2", "A", "A.1" — anything with a digit, or a short
  // all-uppercase token (appendix numbering uses A/B/C without digits).
  let looks_like_section_id = number.chars().any(|ch| ch.is_ascii_digit())
    || number.chars().all(|ch| ch.is_ascii_uppercase());
  if !looks_like_section_id {
    return false;
  }

  let last = tokens[tokens.len() - 1];
  if last.chars().all(|ch| ch.is_ascii_digit()) {
    return false;
  }
  if !last.chars().next().is_some_and(char::is_uppercase) {
    return false;
  }

  has_wide_gap_between_tokens(trimmed, number, last)
}

fn has_wide_gap_between_tokens(trimmed: &str, first: &str, last: &str) -> bool {
  let Some(first_idx) = trimmed.find(first) else {
    return false;
  };
  let first_end = first_idx + first.len();
  let Some(last_start) = trimmed.rfind(last) else {
    return false;
  };
  if last_start <= first_end {
    return false;
  }
  trimmed[first_end..last_start].chars().filter(|ch| *ch == ' ').count() >= 10
}
