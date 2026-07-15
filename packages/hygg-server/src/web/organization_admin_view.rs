use super::*;

/// Renders the admin organization detail page body: settings, the extension's
/// injected panels, members management, documents, and a delete control.
pub(crate) fn organization_content(
  user: &WebUser,
  org: &repo::organizations::OrganizationRow,
  ext_panels: &str,
  members: &[repo::organizations::OrganizationMember],
  books: &[repo::books::BookRow],
) -> String {
  format!(
    "{settings}{panels}{members}{documents}{danger}",
    settings = settings_panel(user, org),
    panels = ext_panels,
    members = members_panel(user, org, members),
    documents = documents_panel(books),
    danger = danger_panel(user, org),
  )
}

fn settings_panel(
  user: &WebUser,
  org: &repo::organizations::OrganizationRow,
) -> String {
  format!(
    r#"<section class="panel"><h2>{name}</h2>
      <form method="post" action="/app/admin/organizations/{id}" class="stack">
        {csrf}
        <label>Name<input name="name" value="{name}"></label>
        <label>Default member permission{access}</label>
        <button type="submit">Save settings</button>
      </form>
      <p class="muted">Slug: {slug}</p>
    </section>"#,
    name = esc(&org.name),
    id = esc(&org.id),
    csrf = csrf_input(user),
    access = access_select("default_access", &org.default_access, false, None),
    slug = esc(&org.slug),
  )
}

fn members_panel(
  user: &WebUser,
  org: &repo::organizations::OrganizationRow,
  members: &[repo::organizations::OrganizationMember],
) -> String {
  let mut rows = String::new();
  for member in members {
    rows.push_str(&member_row(user, &org.id, member));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"5\">No members.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>Members</h2>
      <form method="post" action="/app/admin/organizations/{id}/members" class="inline-form">
        {csrf}
        <input name="email" type="email" placeholder="User email" required>
        {role}
        <button type="submit">Add member</button>
      </form>
      <table><thead><tr><th>Email</th><th>Name</th><th>Role</th><th>Joined</th><th></th></tr></thead>
      <tbody>{rows}</tbody></table>
    </section>"#,
    id = esc(&org.id),
    csrf = csrf_input(user),
    role = org_role_select(None),
  )
}

fn member_row(
  user: &WebUser,
  org_id: &str,
  member: &repo::organizations::OrganizationMember,
) -> String {
  format!(
    r#"<tr><td>{email}</td><td>{name}</td>
      <td><form method="post" action="/app/admin/organizations/{org}/members/{uid}/role" class="inline-form">{csrf}{role}<button type="submit">Set</button></form></td>
      <td>{joined}</td>
      <td><form method="post" action="/app/admin/organizations/{org}/members/{uid}/remove" class="inline-form">{csrf}<button type="submit">Remove</button></form></td>
    </tr>"#,
    email = esc(&member.email),
    name = esc(&member.display_name),
    org = esc(org_id),
    uid = esc(&member.user_id),
    csrf = csrf_input(user),
    role = org_role_select(Some(&member.role)),
    joined = esc(&format_millis(member.created_at)),
  )
}

fn documents_panel(books: &[repo::books::BookRow]) -> String {
  let mut rows = String::new();
  for book in books {
    rows.push_str(&format!(
      "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
      esc(&book.title),
      esc(&book.format),
      book.size_bytes,
      esc(&book.content_hash),
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"4\">No organization documents.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>Documents</h2>
      <table><thead><tr><th>Title</th><th>Format</th><th>Bytes</th><th>Document id</th></tr></thead>
      <tbody>{rows}</tbody></table>
    </section>"#
  )
}

fn danger_panel(
  user: &WebUser,
  org: &repo::organizations::OrganizationRow,
) -> String {
  format!(
    r#"<section class="panel"><h2>Danger zone</h2>
      <form method="post" action="/app/admin/organizations/{id}/delete" class="inline-form">
        {csrf}
        <button type="submit">Delete organization</button>
      </form>
      <p class="muted">Documents revert to their owners' private libraries.</p>
    </section>"#,
    id = esc(&org.id),
    csrf = csrf_input(user),
  )
}

fn org_role_select(selected: Option<&str>) -> String {
  let selected = match selected {
    Some("owner") => "owner",
    _ => "member",
  };
  let mut html = String::from(r#"<select name="role">"#);
  for (value, label) in [("member", "Member"), ("owner", "Owner")] {
    html.push_str(&format!(
      r#"<option value="{}"{}>{}</option>"#,
      value,
      if value == selected { " selected" } else { "" },
      label,
    ));
  }
  html.push_str("</select>");
  html
}
