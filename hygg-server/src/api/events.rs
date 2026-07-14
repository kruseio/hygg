//! SSE push endpoint. Holds a long-lived connection per device and streams a
//! `changed` event whenever another of the user's devices pushes ops, so peers
//! pull near-instantly instead of waiting for their periodic poll. A keep-alive
//! comment every 10s holds the connection open through proxies.
//!
//! Credentials come from headers (the CLI) *or* the query string (the browser
//! PWA/Tauri client, whose `EventSource` cannot set request headers) — see
//! [`EventsAuth`].

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Sse;
use axum::response::sse::{Event, KeepAlive};
use hygg_shared::sync::headers::{MACHINE_ID_HEADER, USER_HEADER};
use hygg_shared::sync::proto::events::{CHANGED, Changed};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

use crate::error::{AppError, AppResult};
use crate::middleware::authn::{
  authenticate, bearer_token, client_ip, header_str,
};
use crate::middleware::entitlement::admit_sync;
use crate::state::AppState;

/// The device credential accepted in the query string. A browser `EventSource`
/// can't set headers, so the SSE endpoint also takes the three-part credential
/// — bearer `token`, account `user` (email), and `machine` id — as query
/// params. Header credentials (the CLI's `ureq` stream) still work and take
/// precedence.
#[derive(Deserialize, Default)]
pub struct EventsAuth {
  token: Option<String>,
  user: Option<String>,
  machine: Option<String>,
}

/// `GET /api/v1/events`
pub async fn events(
  State(state): State<AppState>,
  Query(auth): Query<EventsAuth>,
  headers: HeaderMap,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
  // Prefer explicit header credentials (the CLI), falling back to the query
  // string (the browser client). Either way the full triple is required and
  // verified by the same path every other authenticated request uses.
  let token =
    bearer_token(&headers).or(auth.token).ok_or(AppError::Unauthorized)?;
  let user = header_str(&headers, USER_HEADER).or(auth.user);
  let machine = header_str(&headers, MACHINE_ID_HEADER).or(auth.machine);
  let ip = client_ip(&headers);
  let principal =
    authenticate(&state, &token, user.as_deref(), machine.as_deref(), &ip)
      .await?;
  let principal = admit_sync(&state, principal).await?;

  let rx = state.events.subscribe(&principal.tenant_id, &principal.user_id);
  let stream = BroadcastStream::new(rx).map(|result| {
    // A real change carries the server_time; a lag (missed pings) still tells
    // the client to pull and catch up (server_time 0). Either way: emit the
    // typed `changed` event.
    let server_time = result.unwrap_or(0);
    let event = Event::default()
      .event(CHANGED)
      .json_data(Changed { server_time })
      .expect("Changed serialises");
    Ok(event)
  });
  Ok(Sse::new(stream).keep_alive(
    KeepAlive::new().interval(Duration::from_secs(10)).text("ping"),
  ))
}
