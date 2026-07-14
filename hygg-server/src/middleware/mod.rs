//! Request middleware. `authn` resolves a bearer token into a [`Principal`]
//! extractor; `entitlement` adds the [`SyncPrincipal`] gate on sync endpoints.
//! Later phases add rate-limit and CSRF.
//!
//! [`Principal`]: crate::auth::Principal
//! [`SyncPrincipal`]: crate::middleware::entitlement::SyncPrincipal

pub mod authn;
pub mod entitlement;
