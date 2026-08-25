//! The per-user end-to-end encryption control on the account page.
//!
//! Each user turns encryption on or off for *their own* account here. The
//! server never holds a key, so "on" only *requires* encryption: the marker is
//! flipped and the server starts refusing plaintext uploads, then the user's
//! first client generates the key and the others adopt it. "Off" stops the
//! server enforcing; the clients decrypt and re-upload their documents.

use super::*;

/// The account-page panel. `enabled` is the marker flag; `initialized` is
/// whether a client has published a key yet (a non-empty salt).
pub(crate) fn encryption_panel(
  user: &WebUser,
  enabled: bool,
  initialized: bool,
) -> String {
  let (status_class, status_title, body, action, button) = if enabled {
    let detail = if initialized {
      "On. Your documents and notes are stored encrypted; the server can't \
       read them. Set the key up on each device to read here."
    } else {
      "Required. The next client you connect will generate the key — until \
       then, uploads are refused."
    };
    (
      "status-enabled",
      "Encryption on",
      detail,
      "disable",
      "Turn off encryption",
    )
  } else {
    (
      "status-disabled",
      "Encryption off",
      "Off. Your uploaded documents and notes are readable by the server. \
       Turn it on to require end-to-end encryption for this account.",
      "require",
      "Require end-to-end encryption",
    )
  };
  format!(
    r#"<section class="panel account-card">
      <div class="account-card-header">
        <div><p class="eyebrow">Security</p><h2>End-to-end encryption</h2></div>
        <span class="status-pill {status_class}">{status_title}</span>
      </div>
      <p class="account-summary">{body}</p>
      <div class="account-security">
        <form method="post" action="/account/encryption" class="account-security-form">
          {csrf}
          <input type="hidden" name="action" value="{action}">
          <button type="submit">{icon}<span>{button}</span></button>
        </form>
      </div>
    </section>"#,
    csrf = csrf_input(user),
    icon = icon("lock-keyhole"),
  )
}

/// `POST /account/encryption` — require or disable encryption for the caller's
/// own account.
pub(crate) async fn account_encryption_post(
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
  match form.get("action").map(String::as_str).unwrap_or("") {
    "require" => {
      let _ = repo::encryption::mandate(
        &state.db.conn,
        &user.tenant_id,
        &user.user_id,
      )
      .await;
    }
    "disable" => {
      let _ = repo::encryption::disable(
        &state.db.conn,
        &user.tenant_id,
        &user.user_id,
      )
      .await;
    }
    _ => return error_page(StatusCode::BAD_REQUEST, "Unknown action"),
  }
  Redirect::to("/account").into_response()
}
