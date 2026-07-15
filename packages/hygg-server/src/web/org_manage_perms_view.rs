use std::collections::HashMap;

use super::*;
use crate::repo::directories::DirectoryRow;
use crate::repo::groups::{GroupMember, GroupRow};
use crate::repo::organizations::OrganizationMember;
use crate::repo::permissions::PermissionRow;

/// The permission grants editor: a grant form (subject × target × access) plus
/// a table of current grants with remove controls.
#[allow(clippy::too_many_arguments)]
pub(crate) fn permissions_section(
  user: &WebUser,
  org_id: &str,
  members: &[OrganizationMember],
  groups: &[(GroupRow, Vec<GroupMember>)],
  books: &[repo::books::BookRow],
  directories: &[DirectoryRow],
  perms: &[PermissionRow],
) -> String {
  let user_names: HashMap<&str, &str> =
    members.iter().map(|m| (m.user_id.as_str(), m.email.as_str())).collect();
  let group_names: HashMap<&str, &str> =
    groups.iter().map(|(g, _)| (g.id.as_str(), g.name.as_str())).collect();
  let book_names: HashMap<&str, &str> =
    books.iter().map(|b| (b.content_hash.as_str(), b.title.as_str())).collect();
  let dir_names: HashMap<&str, &str> =
    directories.iter().map(|d| (d.id.as_str(), d.name.as_str())).collect();

  let mut rows = String::new();
  for perm in perms {
    let subject =
      label(&perm.subject_type, &perm.subject_id, &user_names, &group_names);
    let target =
      label(&perm.target_type, &perm.target_id, &book_names, &dir_names);
    rows.push_str(&format!(
      r#"<tr><td>{subject}</td><td>{target}</td><td>{access}</td>
        <td><form method="post" action="/app/organizations/{org}/permissions/remove" class="inline-form">{csrf}<input type="hidden" name="subject" value="{stype}:{sid}"><input type="hidden" name="target" value="{ttype}:{tid}"><button type="submit">Remove</button></form></td>
      </tr>"#,
      subject = esc(&subject),
      target = esc(&target),
      access = esc(access_label(&perm.access)),
      org = esc(org_id),
      csrf = csrf_input(user),
      stype = esc(&perm.subject_type),
      sid = esc(&perm.subject_id),
      ttype = esc(&perm.target_type),
      tid = esc(&perm.target_id),
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"4\">No grants. Members use the default permission.</td></tr>");
  }

  format!(
    r#"<section class="panel"><h2>Permissions</h2>
      <form method="post" action="/app/organizations/{org}/permissions" class="inline-form">
        {csrf}
        {subject}
        {target}
        {access}
        <button type="submit">Grant</button>
      </form>
      <table><thead><tr><th>Subject</th><th>Document / directory</th><th>Access</th><th></th></tr></thead>
      <tbody>{rows}</tbody></table>
    </section>"#,
    org = esc(org_id),
    csrf = csrf_input(user),
    subject = subject_select(members, groups),
    target = target_select(books, directories),
    access = access_select("access", "read_write", false, None),
  )
}

fn label(
  kind: &str,
  id: &str,
  primary: &HashMap<&str, &str>,
  secondary: &HashMap<&str, &str>,
) -> String {
  match kind {
    "user" | "document" => primary.get(id).copied().unwrap_or(id).to_string(),
    _ => format!(
      "{}: {}",
      folder_or_group(kind),
      secondary.get(id).copied().unwrap_or(id)
    ),
  }
}

fn folder_or_group(kind: &str) -> &'static str {
  if kind == "group" { "Group" } else { "Folder" }
}

fn subject_select(
  members: &[OrganizationMember],
  groups: &[(GroupRow, Vec<GroupMember>)],
) -> String {
  let mut html = String::from(r#"<select name="subject" required>"#);
  for member in members {
    html.push_str(&format!(
      r#"<option value="user:{}">{}</option>"#,
      esc(&member.user_id),
      esc(&member.email),
    ));
  }
  for (group, _) in groups {
    html.push_str(&format!(
      r#"<option value="group:{}">Group: {}</option>"#,
      esc(&group.id),
      esc(&group.name),
    ));
  }
  html.push_str("</select>");
  html
}

fn target_select(
  books: &[repo::books::BookRow],
  directories: &[DirectoryRow],
) -> String {
  let mut html = String::from(r#"<select name="target" required>"#);
  for book in books {
    html.push_str(&format!(
      r#"<option value="document:{}">{}</option>"#,
      esc(&book.content_hash),
      esc(&book.title),
    ));
  }
  for dir in directories {
    html.push_str(&format!(
      r#"<option value="directory:{}">Folder: {}</option>"#,
      esc(&dir.id),
      esc(&dir.name),
    ));
  }
  html.push_str("</select>");
  html
}
