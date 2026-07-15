use super::*;

/// Admin-only list of every organization in the tenant + the create wizard.
pub(crate) async fn organizations_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let orgs =
    repo::organizations::list_for_tenant(&state.db.conn, &user.tenant_id)
      .await
      .unwrap_or_default();
  let users = repo::users::list_for_tenant(&state.db.conn, &user.tenant_id)
    .await
    .unwrap_or_default();
  let mut rows = String::new();
  for org in &orgs {
    rows.push_str(&format!(
      r#"<tr><td><a href="/app/admin/organizations/{}">{}</a></td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
      esc(&org.id),
      esc(&org.name),
      esc(&org.slug),
      org.member_count,
      esc(access_label(&org.default_access)),
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"4\">No organizations yet.</td></tr>");
  }
  page(
    "Admin organizations",
    Some(&user),
    format!(
      r#"{wizard}
      <section class="panel"><h2>Organizations</h2>
        <table><thead><tr><th>Name</th><th>Slug</th><th>Members</th><th>Default access</th></tr></thead>
        <tbody>{rows}</tbody></table>
      </section>"#,
      wizard = create_wizard(&user, &users, &state.web_ext.org_create_fields()),
    ),
  )
}

/// The create-organization wizard: names the org and picks the first owner.
/// The web extension may inject extra fields, consumed by its `on_org_created`
/// hook.
fn create_wizard(
  user: &WebUser,
  users: &[repo::users::UserSummary],
  extra_fields: &str,
) -> String {
  format!(
    r#"<section class="panel"><h2>Create organization</h2>
      <form method="post" action="/app/admin/organizations" class="stack">
        {csrf}
        <label>Organization name<input name="name" required></label>
        <label>Owner{owner}</label>
        {extra_fields}
        <button type="submit">Create organization</button>
      </form>
    </section>"#,
    csrf = csrf_input(user),
    owner = user_select("owner_user_id", users),
  )
}

fn user_select(name: &str, users: &[repo::users::UserSummary]) -> String {
  let mut html = format!(r#"<select name="{}" required>"#, esc(name));
  for u in users {
    if u.disabled != 0 {
      continue;
    }
    let label = if u.display_name.trim().is_empty() {
      u.email.clone()
    } else {
      format!("{} ({})", u.display_name, u.email)
    };
    html.push_str(&format!(
      r#"<option value="{}">{}</option>"#,
      esc(&u.id),
      esc(&label)
    ));
  }
  html.push_str("</select>");
  html
}

pub(crate) async fn organization_create_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let user = match require_admin_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let name = field(&form, "name");
  let owner_id = field(&form, "owner_user_id");
  if name.is_empty() || owner_id.is_empty() {
    return Redirect::to("/app/admin/organizations").into_response();
  }
  let Ok(Some(owner)) =
    repo::users::find_by_id(&state.db.conn, &user.tenant_id, &owner_id).await
  else {
    return error_page(StatusCode::NOT_FOUND, "Owner not found");
  };
  match repo::organizations::create(
    &state.db.conn,
    &user.tenant_id,
    &name,
    &owner.id,
  )
  .await
  {
    Ok(id) => {
      // Let the web extension provision whatever it injected fields for.
      state.web_ext.on_org_created(&user, &id, &form).await;
      Redirect::to(&format!("/app/admin/organizations/{id}")).into_response()
    }
    Err(_) => {
      error_page(StatusCode::BAD_REQUEST, "Organization could not be created")
    }
  }
}
