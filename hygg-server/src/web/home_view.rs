use super::*;

/// Per-organization shared-pool storage meters, shown separately from the
/// caller's personal storage. Empty when the user belongs to no organizations.
pub(crate) fn org_storage_panel(orgs: &[(String, i64, Option<i64>)]) -> String {
  if orgs.is_empty() {
    return String::new();
  }
  let mut rows = String::new();
  for (name, used, limit) in orgs {
    let body = match limit {
      Some(limit) => {
        let pct = percent(*used, *limit).clamp(0, 100);
        format!(
          r#"<div class="storage-meter-head"><span>{}</span><span>{} of {} · {}%</span></div>
            <div class="bar"><span style="width:{}%"></span></div>"#,
          esc(name),
          format_bytes(*used),
          format_bytes(*limit),
          pct,
          pct,
        )
      }
      None => format!(
        r#"<div class="storage-meter-head"><span>{}</span><span>{} used</span></div>"#,
        esc(name),
        format_bytes(*used),
      ),
    };
    rows.push_str(&format!(r#"<div class="storage-meter">{body}</div>"#));
  }
  format!(
    r#"<section class="panel"><h2>Organization storage</h2>{rows}</section>"#
  )
}

/// A storage usage meter for the library panel: a progress bar of document
/// bytes against the reported limit (when one applies), with a
/// document/metadata breakdown beneath. Without a limit it shows total used.
pub(crate) fn storage_meter(
  document_bytes: i64,
  metadata_bytes: i64,
  limit: Option<i64>,
) -> String {
  let breakdown = format!(
    "Documents {} · Metadata {}",
    format_bytes(document_bytes),
    format_bytes(metadata_bytes),
  );
  match limit {
    Some(limit) => {
      let pct = percent(document_bytes, limit).clamp(0, 100);
      format!(
        r#"<div class="storage-meter">
          <div class="storage-meter-head"><span>Storage</span><span>{} of {} · {}%</span></div>
          <div class="bar"><span style="width:{}%"></span></div>
          <p class="muted">{}</p>
        </div>"#,
        format_bytes(document_bytes),
        format_bytes(limit),
        pct,
        pct,
        breakdown,
      )
    }
    None => format!(
      r#"<div class="storage-meter">
        <div class="storage-meter-head"><span>Storage</span><span>{} used</span></div>
        <p class="muted">{}</p>
      </div>"#,
      format_bytes(document_bytes + metadata_bytes),
      breakdown,
    ),
  }
}
