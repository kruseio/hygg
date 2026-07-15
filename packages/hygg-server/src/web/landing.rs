//! The application home (`/app/home`). The core serves no page at `/` — an
//! embedder is free to merge its own there — so this file keeps only the
//! authenticated home page.

use super::*;

pub(crate) async fn home_page(
  State(state): State<AppState>,
  headers: HeaderMap,
  axum::extract::Query(query): axum::extract::Query<LibraryQuery>,
) -> Response {
  let user = match require_workspace_user_response(&state, &headers).await {
    Ok(user) => user,
    Err(response) => return response,
  };
  dashboard(&state, user, query).await
}
