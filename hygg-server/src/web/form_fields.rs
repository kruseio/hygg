use super::*;

pub fn format_price(price_cents: i64, currency: &str) -> String {
  let amount = price_cents as f64 / 100.0;
  if currency.eq_ignore_ascii_case("USD") {
    format!("${amount:.2}")
  } else {
    format!("{} {amount:.2}", currency)
  }
}

pub(crate) fn organization_select(
  name: &str,
  selected: Option<&str>,
  organizations: &[repo::organizations::OrganizationMembership],
) -> String {
  let mut html = format!(r#"<select name="{}">"#, esc(name));
  html.push_str(&format!(
    r#"<option value=""{}>Private</option>"#,
    if selected.is_none() { " selected" } else { "" }
  ));
  for org in organizations {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      esc(&org.id),
      if selected == Some(org.id.as_str()) { " selected" } else { "" },
      esc(&org.name)
    ));
  }
  html.push_str("</select>");
  html
}

pub(crate) fn password_status_radios(
  password_enabled: bool,
  has_valid_passkey: bool,
) -> String {
  format!(
    r#"<div class="segmented-radio">
      <label><input type="radio" name="password_enabled" value="enabled"{}><span>Enabled</span></label>
      <label><input type="radio" name="password_enabled" value="disabled"{}{} title="A valid passkey is required"><span>Disabled</span></label>
    </div>"#,
    if password_enabled { " checked" } else { "" },
    if !password_enabled { " checked" } else { "" },
    if has_valid_passkey { "" } else { " disabled" },
  )
}

pub(crate) fn access_overrides_from_form(
  form: &HashMap<String, String>,
) -> Vec<(String, String)> {
  form
    .iter()
    .filter_map(|(key, value)| {
      let book_id = key.strip_prefix("book_access:")?.trim();
      let access = value.trim();
      if book_id.is_empty() || access.is_empty() {
        return None;
      }
      Some((book_id.to_string(), normalized_access(access).to_string()))
    })
    .collect()
}

pub(crate) fn normalized_access(value: &str) -> &'static str {
  match value {
    "read" => "read",
    "none" => "none",
    _ => "read_write",
  }
}

pub(crate) fn access_label(access: &str) -> &'static str {
  match access {
    "read" => "Read only",
    "none" => "No access",
    _ => "Read/write",
  }
}

pub(crate) fn access_select(
  name: &str,
  selected: &str,
  include_default: bool,
  default_access: Option<&str>,
) -> String {
  let mut html = format!(r#"<select name="{}">"#, esc(name));
  if include_default {
    let label =
      default_access.map(access_label).unwrap_or("device default access");
    html.push_str(&format!(
      r#"<option value=""{}>Use default ({})</option>"#,
      if selected.is_empty() { " selected" } else { "" },
      esc(label)
    ));
  }
  for (value, label) in
    [("read_write", "Read/write"), ("read", "Read only"), ("none", "No access")]
  {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      value,
      if selected == value { " selected" } else { "" },
      label
    ));
  }
  html.push_str("</select>");
  html
}

/// The per-document sync ceiling dropdown (`full` | `metadata` | `off`). The
/// account-wide policy each device clamps its local preference against.
pub(crate) fn sync_mode_select(name: &str, selected: &str) -> String {
  let selected = hygg_shared::sync::SyncMode::from_token_or_default(selected);
  let mut html = format!(r#"<select name="{}">"#, esc(name));
  for (value, label) in [
    ("full", "Full — file, position & notes"),
    ("metadata", "Metadata only — position & notes, no file"),
    ("off", "Off — don't sync"),
  ] {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      value,
      if selected.as_str() == value { " selected" } else { "" },
      label
    ));
  }
  html.push_str("</select>");
  html
}

pub(crate) fn normalized_role(value: &str) -> &'static str {
  if value == "admin" { "admin" } else { "user" }
}

pub(crate) fn role_select(selected: &str) -> String {
  let selected = normalized_role(selected);
  let options = ["user", "admin"]
    .iter()
    .map(|role| {
      format!(
        r#"<option value="{}"{}>{}</option>"#,
        role,
        if *role == selected { " selected" } else { "" },
        role_label(role)
      )
    })
    .collect::<String>();
  format!(r#"<select name="role">{options}</select>"#)
}

pub(crate) fn role_label(role: &str) -> &'static str {
  if role == "admin" { "Admin" } else { "User" }
}
