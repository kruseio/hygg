//! The single error type returned by handlers. Maps to an HTTP status plus a
//! small JSON body, and deliberately never leaks internal/database detail to
//! clients (those are logged server-side and surfaced as a generic 500).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// A refusal an extension asked for, carrying its own wording and an optional
/// link to somewhere the caller can resolve it.
///
/// The core never writes this text: an extension that declines a request owns
/// the explanation, and the core only relays it (as the 403 body, or as an
/// error page). That keeps deployment-specific vocabulary out of the core and
/// lets a client render any refusal without knowing what it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial {
  /// Shown to the caller verbatim.
  pub message: String,
  /// Where the caller can act on it, if anywhere.
  pub action_url: Option<String>,
  /// The link's text. Ignored unless `action_url` is set.
  pub action_label: Option<String>,
}

impl Denial {
  /// A refusal with wording but nowhere to go.
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into(), action_url: None, action_label: None }
  }

  /// Attach a link the caller can follow to resolve the refusal.
  pub fn with_action(
    mut self,
    url: impl Into<String>,
    label: impl Into<String>,
  ) -> Self {
    self.action_url = Some(url.into());
    self.action_label = Some(label.into());
    self
  }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
  #[error("not found")]
  NotFound,
  #[error("unauthorized")]
  Unauthorized,
  #[error("forbidden")]
  Forbidden,
  /// Refused by an extension, which supplied the wording. Rendered like
  /// [`AppError::Forbidden`] but with the extension's message and link.
  #[error("{}", .0.message)]
  Denied(Denial),
  #[error("bad request: {0}")]
  BadRequest(String),
  #[error("conflict: {0}")]
  Conflict(String),
  #[error("too many requests")]
  TooManyRequests,
  #[error("internal error")]
  Internal,
}

impl AppError {
  fn status(&self) -> StatusCode {
    match self {
      AppError::NotFound => StatusCode::NOT_FOUND,
      AppError::Unauthorized => StatusCode::UNAUTHORIZED,
      AppError::Forbidden | AppError::Denied(_) => StatusCode::FORBIDDEN,
      AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
      AppError::Conflict(_) => StatusCode::CONFLICT,
      AppError::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
      AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

impl IntoResponse for AppError {
  fn into_response(self) -> Response {
    let status = self.status();
    // A denial carries the extension's own link alongside its message, so a
    // client can offer the caller somewhere to go without knowing the reason.
    // Serialised through the shared DTO the clients parse, so the two can't
    // drift apart silently.
    if let AppError::Denied(d) = self {
      let body = axum::Json(hygg_shared::sync::proto::DenialBody {
        error: d.message,
        action_url: d.action_url,
        action_label: d.action_label,
      });
      return (status, body).into_response();
    }
    // Internal errors are intentionally opaque to the client.
    let message = match &self {
      AppError::Internal => "internal error".to_string(),
      other => other.to_string(),
    };
    let body = axum::Json(json!({ "error": message }));
    (status, body).into_response()
  }
}

impl From<sea_orm::DbErr> for AppError {
  fn from(err: sea_orm::DbErr) -> Self {
    tracing::error!("database error: {err}");
    AppError::Internal
  }
}

impl From<argon2::password_hash::Error> for AppError {
  fn from(err: argon2::password_hash::Error) -> Self {
    tracing::error!("password hashing error: {err}");
    AppError::Internal
  }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
  use super::*;
  use axum::http::StatusCode;

  #[test]
  fn statuses_map_as_expected() {
    assert_eq!(AppError::NotFound.status(), StatusCode::NOT_FOUND);
    assert_eq!(AppError::Unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(AppError::Forbidden.status(), StatusCode::FORBIDDEN);
    assert_eq!(AppError::Internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
  }

  #[test]
  fn a_denial_answers_403_like_a_plain_forbidden() {
    let denial = Denial::new("no").with_action("/somewhere", "Go");
    assert_eq!(AppError::Denied(denial).status(), StatusCode::FORBIDDEN);
  }

  #[test]
  fn a_denial_carries_the_extensions_wording() {
    let denial = Denial::new("not included here").with_action("/x", "See more");
    assert_eq!(
      AppError::Denied(denial.clone()).to_string(),
      "not included here"
    );
    assert_eq!(denial.action_url.as_deref(), Some("/x"));
    assert_eq!(denial.action_label.as_deref(), Some("See more"));
  }

  #[test]
  fn a_denial_needs_no_action_link() {
    let denial = Denial::new("nope");
    assert_eq!(denial.action_url, None);
    assert_eq!(denial.action_label, None);
  }
}
