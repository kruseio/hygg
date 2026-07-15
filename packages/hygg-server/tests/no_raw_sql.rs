//! This crate builds its queries; it does not write them.
//!
//! The port to an ORM removed every SQL string and every direct sqlx call. Both
//! are easy to reintroduce — a query builder is more work than a string when
//! you are in a hurry, and the escape hatches are one call away — and neither
//! shows up as a compile error. So this reads the source and says no.
//!
//! If a query genuinely cannot be expressed through the builder, that is worth
//! a conversation rather than a quiet exception: raise it, don't silence this.

use std::fs;
use std::path::{Path, PathBuf};

/// Fragments that only appear in a hand-written query. Matched
/// case-insensitively against string literals, and paired so that prose
/// mentioning one word ("Select" on a button, "update the row") does not trip
/// them — a real statement has the shape, not just the keyword.
const SQL_SHAPES: &[(&str, &str)] = &[
  ("select ", " from "),
  ("insert into ", " values"),
  ("insert into ", " select"),
  ("update ", " set "),
  ("delete from ", ""),
  ("create table", ""),
  ("alter table", ""),
];

/// Ways back to the raw driver, whatever the string situation.
const ESCAPE_HATCHES: &[&str] = &[
  "sqlx::",
  "Statement::from_string",
  "Statement::from_sql_and_values",
  "execute_unprepared",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      rust_files(&path, out);
    } else if path.extension().is_some_and(|e| e == "rs") {
      out.push(path);
    }
  }
}

/// String literals on a line, lowercased. Crude on purpose: it over-reports
/// rather than miss a query, and the pairing above keeps that tolerable.
fn literals(line: &str) -> Vec<String> {
  let mut out = Vec::new();
  let mut rest = line;
  while let Some(start) = rest.find('"') {
    let after = &rest[start + 1..];
    let Some(end) = after.find('"') else { break };
    out.push(after[..end].to_lowercase());
    rest = &after[end + 1..];
  }
  out
}

fn src_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn no_sql_statements_in_string_literals() {
  let mut files = Vec::new();
  rust_files(&src_dir(), &mut files);
  assert!(!files.is_empty(), "found no sources to check");

  let mut offenders = Vec::new();
  for file in &files {
    let Ok(text) = fs::read_to_string(file) else { continue };
    for (n, line) in text.lines().enumerate() {
      for literal in literals(line) {
        for (head, tail) in SQL_SHAPES {
          if literal.contains(head) && literal.contains(tail) {
            offenders.push(format!(
              "{}:{}: {}",
              file.display(),
              n + 1,
              line.trim()
            ));
          }
        }
      }
    }
  }
  assert!(
    offenders.is_empty(),
    "SQL belongs in the schema builder or the query builder, not a string:\n{}",
    offenders.join("\n")
  );
}

#[test]
fn no_direct_driver_use() {
  let mut files = Vec::new();
  rust_files(&src_dir(), &mut files);

  let mut offenders = Vec::new();
  for file in &files {
    let Ok(text) = fs::read_to_string(file) else { continue };
    for (n, line) in text.lines().enumerate() {
      // The doc comments in this file name the hatches they forbid.
      if line.trim_start().starts_with("//") {
        continue;
      }
      for hatch in ESCAPE_HATCHES {
        if line.contains(hatch) {
          offenders.push(format!(
            "{}:{}: {}",
            file.display(),
            n + 1,
            line.trim()
          ));
        }
      }
    }
  }
  assert!(
    offenders.is_empty(),
    "reach for the ORM, not the driver underneath it:\n{}",
    offenders.join("\n")
  );
}

#[test]
fn the_check_can_actually_see_sql() {
  // A guard that cannot fail is worse than none: it reads as protection.
  let sample = r#"    let q = "SELECT id FROM users WHERE tenant_id = ?";"#;
  let found = literals(sample)
    .iter()
    .any(|l| SQL_SHAPES.iter().any(|(h, t)| l.contains(h) && l.contains(t)));
  assert!(found, "the matcher missed an obvious SELECT");

  // And it must not trip on prose that merely contains a keyword.
  let prose = r#"        if (label) label.textContent = "Select";"#;
  let tripped = literals(prose)
    .iter()
    .any(|l| SQL_SHAPES.iter().any(|(h, t)| l.contains(h) && l.contains(t)));
  assert!(!tripped, "the matcher tripped on a button label");
}
