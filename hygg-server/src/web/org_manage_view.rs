use super::*;
use crate::repo::directories::DirectoryRow;
use crate::repo::groups::{GroupMember, GroupRow};
use crate::repo::organizations::{OrganizationMember, OrganizationRow};
use crate::repo::permissions::PermissionRow;

/// Live usage counts for the org (seats/storage/devices used).
pub(crate) struct OrgUsage {
  pub seats: i64,
  pub storage: i64,
  pub devices: i64,
}

/// Owner-facing management page body: the extension's injected panels, default
/// permission, directories, documents, groups, and the permission grants
/// editor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn org_manage_content(
  user: &WebUser,
  org: &OrganizationRow,
  ext_panels: &str,
  members: &[OrganizationMember],
  directories: &[DirectoryRow],
  groups: &[(GroupRow, Vec<GroupMember>)],
  books: &[repo::books::BookRow],
  perms: &[PermissionRow],
) -> String {
  format!(
    "{panels}{settings}{dirs}{docs}{groups}{perms}",
    panels = ext_panels,
    settings = settings_section(user, org),
    dirs = directories_section(user, &org.id, directories),
    docs = documents_section(user, &org.id, books, directories),
    groups = groups_section(user, &org.id, groups),
    perms = permissions_section(
      user,
      &org.id,
      members,
      groups,
      books,
      directories,
      perms,
    ),
  )
}

fn settings_section(user: &WebUser, org: &OrganizationRow) -> String {
  format!(
    r#"<section class="panel"><h2>{name}</h2>
      <form method="post" action="/app/organizations/{id}/default-access" class="inline-form">
        {csrf}
        <label>Default member permission {access}</label>
        <button type="submit">Save</button>
      </form>
    </section>"#,
    name = esc(&org.name),
    id = esc(&org.id),
    csrf = csrf_input(user),
    access = access_select("default_access", &org.default_access, false, None),
  )
}

fn directories_section(
  user: &WebUser,
  org_id: &str,
  directories: &[DirectoryRow],
) -> String {
  let names: std::collections::HashMap<&str, &str> =
    directories.iter().map(|d| (d.id.as_str(), d.name.as_str())).collect();
  let mut rows = String::new();
  for dir in directories {
    let parent = dir
      .parent_id
      .as_deref()
      .and_then(|id| names.get(id).copied())
      .unwrap_or("—");
    rows.push_str(&format!(
      "<tr><td>{}</td><td>{}</td></tr>",
      esc(&dir.name),
      esc(parent),
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"2\">No directories.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>Directories</h2>
      <form method="post" action="/app/organizations/{id}/directories" class="inline-form">
        {csrf}
        <input name="name" placeholder="Directory name" required>
        <label>Parent {parent}</label>
        <button type="submit">Create directory</button>
      </form>
      <table><thead><tr><th>Name</th><th>Parent</th></tr></thead>
      <tbody>{rows}</tbody></table>
    </section>"#,
    id = esc(org_id),
    csrf = csrf_input(user),
    parent = dir_select("parent_id", directories, None, "Top level"),
  )
}

fn documents_section(
  user: &WebUser,
  org_id: &str,
  books: &[repo::books::BookRow],
  directories: &[DirectoryRow],
) -> String {
  let mut rows = String::new();
  for book in books {
    rows.push_str(&format!(
      r#"<tr><td>{title}</td><td>{format}</td>
        <td><form method="post" action="/app/organizations/{org}/documents/{hash}/directory" class="inline-form">{csrf}{dirs}<button type="submit">Move</button></form></td>
      </tr>"#,
      title = esc(&book.title),
      format = esc(&book.format),
      org = esc(org_id),
      hash = esc(&book.content_hash),
      csrf = csrf_input(user),
      dirs = dir_select(
        "directory_id",
        directories,
        book.directory_id.as_deref(),
        "Unfiled",
      ),
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"3\">No documents.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>Documents</h2>
      <table><thead><tr><th>Title</th><th>Format</th><th>Directory</th></tr></thead>
      <tbody>{rows}</tbody></table>
    </section>"#
  )
}

fn groups_section(
  user: &WebUser,
  org_id: &str,
  groups: &[(GroupRow, Vec<GroupMember>)],
) -> String {
  let mut blocks = String::new();
  for (group, members) in groups {
    let mut member_rows = String::new();
    for member in members {
      member_rows.push_str(&format!(
        r#"<li>{email}<form method="post" action="/app/organizations/{org}/groups/{gid}/members/{uid}/remove" class="inline-form">{csrf}<button type="submit">Remove</button></form></li>"#,
        email = esc(&member.email),
        org = esc(org_id),
        gid = esc(&group.id),
        uid = esc(&member.user_id),
        csrf = csrf_input(user),
      ));
    }
    blocks.push_str(&format!(
      r#"<div class="modal-section"><h4>{name}</h4>
        <ul class="plain">{member_rows}</ul>
        <form method="post" action="/app/organizations/{org}/groups/{gid}/members" class="inline-form">{csrf}<input name="email" type="email" placeholder="User email" required><button type="submit">Add</button></form>
      </div>"#,
      name = esc(&group.name),
      member_rows = member_rows,
      org = esc(org_id),
      gid = esc(&group.id),
      csrf = csrf_input(user),
    ));
  }
  format!(
    r#"<section class="panel"><h2>Groups</h2>
      <form method="post" action="/app/organizations/{id}/groups" class="inline-form">
        {csrf}<input name="name" placeholder="Group name" required><button type="submit">Create group</button>
      </form>
      {blocks}
    </section>"#,
    id = esc(org_id),
    csrf = csrf_input(user),
  )
}

pub(crate) fn dir_select(
  name: &str,
  directories: &[DirectoryRow],
  selected: Option<&str>,
  top_label: &str,
) -> String {
  let mut html = format!(r#"<select name="{}">"#, esc(name));
  html.push_str(&format!(
    r#"<option value=""{}>{}</option>"#,
    if selected.is_none() { " selected" } else { "" },
    esc(top_label),
  ));
  for dir in directories {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      esc(&dir.id),
      if selected == Some(dir.id.as_str()) { " selected" } else { "" },
      esc(&dir.name),
    ));
  }
  html.push_str("</select>");
  html
}
