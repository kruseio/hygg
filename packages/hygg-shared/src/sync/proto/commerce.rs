//! Commercial (SaaS) wire shapes: the plans a paid deployment offers and the
//! checkout handoff that carries a bearer-token client into the server's web
//! checkout.
//!
//! A plain self-host server serves none of these routes, so a client treats
//! their absence (a 404 or a network error) as "this deployment has no
//! commerce" and shows nothing — the same optional-by-default stance
//! [`super::MeResponse::label`] takes. The shapes live here, in the permissive
//! shared crate, so the open PWA and the closed SaaS server stay statically
//! typed across the boundary without either's license reaching across it; only
//! a commercial deployment ever implements them.

use serde::{Deserialize, Serialize};

/// One purchasable plan, as returned by `GET /api/v1/commerce/plans`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommercePlan {
  pub slug: String,
  pub name: String,
  pub price_cents: i64,
  pub currency: String,
  /// Personal storage budget in bytes; `0` = unlimited.
  pub storage_bytes: i64,
  /// Device cap; `0` = unlimited.
  pub device_limit: i64,
}

/// `GET /api/v1/commerce/plans` response: the deployment's purchasable plans,
/// cheapest first. An empty list is a valid answer (a deployment with nothing
/// to sell) and reads the same as no commerce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlansResponse {
  pub plans: Vec<CommercePlan>,
}

/// `POST /api/v1/commerce/checkout` request: which plan to start checkout for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutRequest {
  pub tier_slug: String,
}

/// `POST /api/v1/commerce/checkout` response: a one-time URL that logs the
/// bearer-authenticated caller into the server's web checkout for the chosen
/// plan. The client opens it as a top-level navigation — a first-party page
/// load on the server's own origin — which is why the server returns a URL
/// carrying a single-use ticket rather than a body the client would post
/// itself: a cross-origin fetch could never establish the server's web
/// session cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutResponse {
  pub url: String,
}
