//! The `/app/shares` page: an inbox of incoming shares to accept/decline, a
//! form to share one of your personal documents with another user (by email),
//! and an outbox of what you've shared and to whom. The POST handlers live in
//! `shares_actions`; the accept/limit logic is shared via [`share_limit_ctx`].

use super::*;

/// `GET /app/shares`
pub(crate) async fn shares_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  let pool = &state.db.conn;
  let inbox = repo::shares::list_inbox(pool, &user.tenant_id, &user.user_id)
    .await
    .unwrap_or_default();
  let outbox = repo::shares::list_outbox(pool, &user.tenant_id, &user.user_id)
    .await
    .unwrap_or_default();
  let shareable = shareable_documents(&state, &user).await;
  let limit = state
    .entitlements
    .share_limit(crate::ext::EntCtx {
      tenant_id: &user.tenant_id,
      user_id: &user.user_id,
      is_admin: user.is_admin(),
    })
    .await;
  let outgoing =
    repo::shares::outgoing_active_count(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or(0);
  let incoming =
    repo::shares::incoming_accepted_count(pool, &user.tenant_id, &user.user_id)
      .await
      .unwrap_or(0);

  let body = format!(
    r#"<section class="panel">
      <div class="panel-head"><h2>Incoming shares</h2>{incoming_meta}</div>
      {inbox}
    </section>
    <section class="panel">
      <div class="panel-head"><h2>Share a document</h2></div>
      {form}
    </section>
    <section class="panel">
      <div class="panel-head"><h2>Documents you've shared</h2>{outgoing_meta}</div>
      {outbox}
    </section>"#,
    incoming_meta = meter("Received", incoming, limit),
    outgoing_meta = meter("Shared out", outgoing, limit),
    inbox = render_inbox(&user, &inbox),
    form = render_share_form(&user, &shareable),
    outbox = render_outbox(&user, &outbox),
  );
  page("Shared documents", Some(&user), body)
}

/// The caller's own personal (non-organization) documents — the only documents
/// they may share directly with another user.
async fn shareable_documents(
  state: &AppState,
  user: &WebUser,
) -> Vec<repo::books::BookRow> {
  repo::books::list_for_user(&state.db.conn, &user.tenant_id, &user.user_id)
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|b| b.owner_user_id == user.user_id && b.organization_id.is_none())
    .collect()
}

fn meter(label: &str, used: i64, limit: Option<i64>) -> String {
  let value = match limit {
    Some(limit) => format!("{used} / {limit}"),
    None => used.to_string(),
  };
  format!(r#"<span class="status-pill">{}: {}</span>"#, esc(label), value)
}

fn render_inbox(user: &WebUser, inbox: &[repo::shares::ShareRow]) -> String {
  if inbox.is_empty() {
    return r#"<p class="muted">No incoming shares.</p>"#.to_string();
  }
  let csrf = csrf_input(user);
  inbox
    .iter()
    .map(|s| {
      format!(
        r#"<div class="actions" style="justify-content:space-between">
          <div><strong>{title}</strong><span class="muted"> — from {from} · {access}</span></div>
          <div class="actions">
            <form method="post" action="/app/shares/{id}/accept">{csrf}<button type="submit">Accept</button></form>
            <form method="post" action="/app/shares/{id}/decline">{csrf}<button class="secondary" type="submit">Decline</button></form>
          </div>
        </div>"#,
        title = esc(&doc_label(&s.title, &s.author, &s.content_hash)),
        from = esc(&s.counterparty_email),
        access = esc(access_label(&s.access)),
        id = esc(&s.id),
        csrf = csrf,
      )
    })
    .collect()
}

fn render_share_form(
  user: &WebUser,
  shareable: &[repo::books::BookRow],
) -> String {
  if shareable.is_empty() {
    return r#"<p class="muted">You have no personal documents to share yet. Upload one from the CLI or your library first.</p>"#.to_string();
  }
  let options: String = shareable
    .iter()
    .map(|b| {
      format!(
        r#"<option value="{hash}">{label}</option>"#,
        hash = esc(&b.content_hash),
        label = esc(&doc_label(&b.title, &b.author, &b.content_hash)),
      )
    })
    .collect();
  format!(
    r#"<form method="post" action="/app/shares" class="actions">
      {csrf}
      <select name="content_hash" aria-label="Document">{options}</select>
      <input name="email" type="email" placeholder="Recipient email" autocomplete="off" required>
      <select name="access" aria-label="Access">
        <option value="read">Read only</option>
        <option value="read_write">Read/write</option>
      </select>
      <button type="submit">Share</button>
    </form>"#,
    csrf = csrf_input(user),
    options = options,
  )
}

fn render_outbox(user: &WebUser, outbox: &[repo::shares::ShareRow]) -> String {
  if outbox.is_empty() {
    return r#"<p class="muted">You haven't shared any documents.</p>"#
      .to_string();
  }
  let csrf = csrf_input(user);
  let rows: String = outbox
    .iter()
    .map(|s| {
      let active = s.status == repo::shares::PENDING
        || s.status == repo::shares::ACCEPTED;
      let action = if active {
        format!(
          r#"<form method="post" action="/app/shares/{id}/revoke">{csrf}<button class="secondary" type="submit">Revoke</button></form>"#,
          id = esc(&s.id),
          csrf = csrf,
        )
      } else {
        String::new()
      };
      format!(
        r#"<tr><td>{title}</td><td>{to}</td><td>{access}</td><td><span class="status-pill">{status}</span></td><td>{action}</td></tr>"#,
        title = esc(&doc_label(&s.title, &s.author, &s.content_hash)),
        to = esc(&s.counterparty_email),
        access = esc(access_label(&s.access)),
        status = esc(&s.status),
        action = action,
      )
    })
    .collect();
  format!(
    r#"<table><thead><tr><th>Document</th><th>Shared with</th><th>Access</th><th>Status</th><th></th></tr></thead><tbody>{rows}</tbody></table>"#
  )
}

/// A human label for a document row, falling back to the author or a short hash
/// when the title is empty (e.g. metadata not yet synced).
pub(crate) fn doc_label(
  title: &str,
  author: &str,
  content_hash: &str,
) -> String {
  let title = title.trim();
  if !title.is_empty() {
    return title.to_string();
  }
  let author = author.trim();
  if !author.is_empty() {
    return author.to_string();
  }
  format!("Document {}", &content_hash[..content_hash.len().min(8)])
}
