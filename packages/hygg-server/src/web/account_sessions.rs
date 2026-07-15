use super::*;

pub(crate) async fn account_sessions_page() -> Response {
  Redirect::to("/account").into_response()
}

pub(crate) async fn account_session_revoke_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(session_id): Path<String>,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let revoking_current = session_id == user.session_id;
  let _ = repo::sessions::revoke_for_user(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
    &session_id,
  )
  .await;
  if revoking_current {
    return ([(header::SET_COOKIE, delete_cookie())], Redirect::to("/login"))
      .into_response();
  }
  Redirect::to("/account").into_response()
}

pub(crate) async fn account_sessions_revoke_all_post(
  State(state): State<AppState>,
  headers: HeaderMap,
  Form(form): Form<HashMap<String, String>>,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  if !csrf_ok(&user, &form) {
    return error_page(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }
  let _ = repo::sessions::revoke_all_for_user(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
  )
  .await;
  ([(header::SET_COOKIE, delete_cookie())], Redirect::to("/login"))
    .into_response()
}

pub(crate) fn sessions_content(
  user: &WebUser,
  sessions: &[repo::sessions::SessionSummary],
  token_sessions: &[repo::tokens::ApiTokenSession],
  current_session_id: &str,
  action_prefix: &str,
  show_tokens: bool,
) -> String {
  let mut rows = String::new();
  for session in sessions {
    let marker = if session.id == current_session_id {
      r#"<span class="badge">Current</span>"#
    } else {
      ""
    };
    let action = format!("{}/{}/revoke", action_prefix, session.id);
    rows.push_str(&format!(
      r#"<tr><td class="mono">{}{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>
      <td><form method="post" action="{}">{}<button class="danger" type="submit">Revoke</button></form></td></tr>"#,
      esc(&short_session_id(&session.id)),
      marker,
      esc(&format_millis(session.created_at)),
      esc(&format_millis(session.last_used_at.unwrap_or(session.created_at))),
      esc(&format_millis(session.expires_at)),
      esc(session.ip.as_deref().unwrap_or("")),
      esc(session.user_agent.as_deref().unwrap_or("")),
      esc(&action),
      csrf_input(user)
    ));
  }
  if rows.is_empty() {
    rows
      .push_str("<tr><td colspan=\"7\">No active browser sessions.</td></tr>");
  }

  let revoke_all_action = format!("{action_prefix}/revoke-all");
  let token_note = if show_tokens {
    " Device API tokens do not expire automatically; revoke the device to disable API access."
  } else {
    ""
  };
  // "Sessions" and the browser-session table are one card: heading, the
  // expiry note, the revoke-all action, then the list.
  let mut html = format!(
    r#"<section class="panel"><div class="section-title"><div><h2>Sessions</h2>
      <p class="muted">Browser sessions expire after one full day without activity.{token_note}</p></div>
      <form method="post" action="{}">{}<button class="danger" type="submit">Revoke all sessions</button></form></div>
      <table>
      <thead><tr><th>Session</th><th>Created</th><th>Last activity</th><th>Expires</th><th>IP</th><th>Browser</th><th></th></tr></thead>
      <tbody>{rows}</tbody>
    </table></section>"#,
    esc(&revoke_all_action),
    csrf_input(user)
  );

  if show_tokens {
    let mut token_rows = String::new();
    for token in token_sessions {
      let state = if token.revoked != 0 || token.device_revoked != 0 {
        "revoked"
      } else {
        "active"
      };
      let last_activity = token
        .last_used_at
        .or(token.device_last_seen_at)
        .unwrap_or(token.created_at);
      let expires = token
        .expires_at
        .map(format_millis)
        .unwrap_or_else(|| "Never".to_string());
      token_rows.push_str(&format!(
        r#"<tr><td>{}</td><td class="mono">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
        esc(&token.device_name),
        esc(&token.prefix),
        esc(&token.platform),
        esc(state),
        esc(&format_millis(token.created_at)),
        esc(&format_millis(last_activity)),
        esc(&expires)
      ));
    }
    if token_rows.is_empty() {
      token_rows
        .push_str("<tr><td colspan=\"7\">No device API tokens.</td></tr>");
    }
    html.push_str(&format!(
      r#"<section class="panel"><h2>Device API tokens</h2><table>
      <thead><tr><th>Device</th><th>Token</th><th>Platform</th><th>Status</th><th>Created</th><th>Last activity</th><th>Expires</th></tr></thead>
      <tbody>{token_rows}</tbody>
    </table></section>"#
    ));
  }
  html
}
