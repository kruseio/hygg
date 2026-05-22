use lopdf::content::{Content, Operation};
use lopdf::{Document, Object};
use std::path::Path;

/// Load a PDF and rewrite its content streams so that text operators
/// `'` (move-to-next-line + show) and `"` (set spacing + move-to-next-line +
/// show) are expanded into the equivalent `T*` / `Tw` / `Tc` / `Tj` sequences.
///
/// Several PDF rendering libraries (including the version of `pdf-extract`
/// we depend on) do not implement these operators, so any text emitted via
/// `'` / `"` is silently dropped. Pre-expanding the streams ensures the
/// downstream extractor sees the same text the PDF actually contains.
pub(crate) fn patched_pdf_bytes(
  path: &Path,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
  let mut doc = Document::load(path)?;
  expand_quote_operators(&mut doc);

  let mut bytes = Vec::new();
  doc.save_to(&mut bytes)?;
  Ok(bytes)
}

fn expand_quote_operators(doc: &mut Document) {
  let stream_ids: Vec<_> = doc
    .get_pages()
    .into_values()
    .flat_map(|page_id| doc.get_page_contents(page_id))
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect();

  for stream_id in stream_ids {
    let Ok(data) = doc.get_object(stream_id).and_then(|obj| {
      obj.as_stream().and_then(|s| {
        s.decompressed_content().or_else(|_| Ok(s.content.clone()))
      })
    }) else {
      continue;
    };

    let Ok(content) = Content::decode(&data) else {
      continue;
    };
    if !content.operations.iter().any(needs_expansion) {
      continue;
    }

    let mut rewritten = Vec::with_capacity(content.operations.len() + 4);
    for op in content.operations {
      append_expanded(&mut rewritten, op);
    }
    let new_content = Content { operations: rewritten };
    let Ok(encoded) = new_content.encode() else { continue };

    if let Ok(stream) =
      doc.get_object_mut(stream_id).and_then(Object::as_stream_mut)
    {
      stream.set_plain_content(encoded);
    }
  }
}

fn needs_expansion(op: &Operation) -> bool {
  matches!(op.operator.as_str(), "'" | "\"")
}

fn append_expanded(out: &mut Vec<Operation>, op: Operation) {
  match op.operator.as_str() {
    "'" => {
      // `string '` ≡ `T* string Tj`.
      out.push(Operation::new("T*", Vec::new()));
      out.push(Operation::new("Tj", op.operands));
    }
    "\"" => {
      // `aw ac string "` ≡ `aw Tw ac Tc string '` ≡
      // `aw Tw ac Tc T* string Tj`.
      let mut operands = op.operands.into_iter();
      let aw = operands.next();
      let ac = operands.next();
      let text = operands.next();
      if let Some(aw) = aw {
        out.push(Operation::new("Tw", vec![aw]));
      }
      if let Some(ac) = ac {
        out.push(Operation::new("Tc", vec![ac]));
      }
      out.push(Operation::new("T*", Vec::new()));
      if let Some(text) = text {
        out.push(Operation::new("Tj", vec![text]));
      }
    }
    _ => out.push(op),
  }
}

#[cfg(test)]
mod tests {
  use super::{Object, Operation, append_expanded};
  use lopdf::StringFormat;

  #[test]
  fn expands_single_quote_to_t_star_tj() {
    let op = Operation::new(
      "'",
      vec![Object::String(b"hello".to_vec(), StringFormat::Literal)],
    );
    let mut out = Vec::new();
    append_expanded(&mut out, op);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].operator, "T*");
    assert!(out[0].operands.is_empty());
    assert_eq!(out[1].operator, "Tj");
    assert_eq!(out[1].operands.len(), 1);
  }

  #[test]
  fn expands_double_quote_to_tw_tc_t_star_tj() {
    let op = Operation::new(
      "\"",
      vec![
        Object::Integer(5),
        Object::Integer(2),
        Object::String(b"world".to_vec(), StringFormat::Literal),
      ],
    );
    let mut out = Vec::new();
    append_expanded(&mut out, op);
    assert_eq!(out.len(), 4);
    assert_eq!(out[0].operator, "Tw");
    assert_eq!(out[1].operator, "Tc");
    assert_eq!(out[2].operator, "T*");
    assert_eq!(out[3].operator, "Tj");
  }

  #[test]
  fn passes_other_operators_through_untouched() {
    let op = Operation::new("Tf", vec![Object::Name(b"R1".to_vec())]);
    let mut out = Vec::new();
    append_expanded(&mut out, op);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].operator, "Tf");
  }
}
