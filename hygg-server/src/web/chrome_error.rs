//! Error pages. Split out of [`super::chrome`] to keep each file within the LOC
//! budget.

use super::*;

pub fn error_page(status: StatusCode, message: &str) -> Response {
  (
    status,
    page(
      "Error",
      None,
      format!("<section class=\"panel\"><h1>{}</h1></section>", esc(message)),
    ),
  )
    .into_response()
}

/// Render an [`AppError`] a hook returned. A [`Denial`] is shown in the words
/// the hook chose, with its link when it offered one; anything else falls back
/// to the plain status wording. The core supplies no explanation of its own —
/// it does not know why the request was refused.
pub fn denial_page(err: AppError) -> Response {
  let AppError::Denied(denial) = err else {
    return error_page(StatusCode::FORBIDDEN, "Forbidden");
  };
  let action = match (&denial.action_url, &denial.action_label) {
    (Some(url), Some(label)) => format!(
      r#"<p><a class="button" href="{}">{}</a></p>"#,
      esc(url),
      esc(label)
    ),
    _ => String::new(),
  };
  (
    StatusCode::FORBIDDEN,
    page(
      "Error",
      None,
      format!(
        "<section class=\"panel\"><h1>{}</h1>{action}</section>",
        esc(&denial.message)
      ),
    ),
  )
    .into_response()
}
