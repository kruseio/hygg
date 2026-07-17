//! Dormant commerce client: subscription discovery and the checkout handoff.
//!
//! Compiled into every build but only ever *called* when the app is pointed at
//! an official server (see [`crate::settings::Settings::is_official_server`]) —
//! a self-host build never reaches these. Best-effort like the rest of sync: a
//! plain hygg-server has no commerce routes, so these 404, and the caller reads
//! that as "no commerce" and shows nothing.

use gloo_net::http::Request;
use hygg_shared::sync::proto::{
  CheckoutRequest, CheckoutResponse, CommercePlan, PlansResponse,
};

use super::{Creds, Res, api, authed, error_body};

/// Fetch the plans this server offers for subscription, cheapest first. An
/// error (or an empty list) means "show no commerce".
pub async fn fetch_plans(server: &str) -> Res<Vec<CommercePlan>> {
  let resp = Request::get(&api(server, "/commerce/plans"))
    .send()
    .await
    .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  let body: PlansResponse = resp.json().await.map_err(|e| e.to_string())?;
  Ok(body.plans)
}

/// Start checkout for a plan: ask the server for a one-time URL that logs this
/// user into the web checkout, and return it for the caller to open in a new
/// tab. Requires authenticated credentials — the ticket is minted for the
/// bearer principal.
pub async fn start_checkout(creds: &Creds, tier_slug: &str) -> Res<String> {
  let body = CheckoutRequest { tier_slug: tier_slug.to_string() };
  let resp =
    authed(Request::post(&api(&creds.server, "/commerce/checkout")), creds)
      .json(&body)
      .map_err(|e| e.to_string())?
      .send()
      .await
      .map_err(|e| e.to_string())?;
  if !resp.ok() {
    return Err(error_body(resp).await);
  }
  let body: CheckoutResponse = resp.json().await.map_err(|e| e.to_string())?;
  Ok(body.url)
}
