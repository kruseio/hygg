//! Server-rendered web UI: password/recovery login, signup, user device token
//! creation, and an admin backoffice for users, device permissions, recovery
//! codes, and passkey revocation. No Node build step.

use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use webauthn_rs::prelude::{
  CredentialID, Passkey, PublicKeyCredential, RegisterPublicKeyCredential,
};

use crate::auth::Role;
use crate::auth::password::{hash_password, verify_password};
use crate::auth::token::{generate_token, hash_secret};
use crate::bootstrap::{DEFAULT_TENANT_SLUG, ensure_default_tenant};
use crate::error::{AppError, Denial};
use crate::repo;
use crate::state::{
  AppState, PendingPasskeyAuthentication, PendingPasskeyRegistration,
};
use crate::util::now_millis;

mod account;
mod account_encryption;
mod account_sessions;
mod admin_dashboard;
mod admin_dashboard_panels;
mod admin_devices;
mod admin_misc;
mod admin_users;
mod auth_form;
mod book_tags;
mod books;
mod chrome;
mod chrome_error;
mod chrome_nav;
mod device_tokens;
mod devices;
mod docs;
mod docs_client;
mod docs_render;
mod docs_search;
mod docs_view;
mod form_fields;
mod format_util;
mod guards;
mod home;
mod home_view;
mod landing;
mod library;
mod library_controls;
mod library_view;
mod login;
mod notify;
mod org_manage;
mod org_manage_actions;
mod org_manage_perms;
mod org_manage_perms_view;
mod org_manage_view;
mod organization_admin;
mod organization_admin_view;
mod organization_books;
mod organizations;
mod passkeys_api;
mod passkeys_ui;
mod request_util;
mod session;
mod shares;
mod shares_actions;
mod signup;
mod style;

pub(crate) use account::*;
pub(crate) use account_encryption::*;
pub(crate) use account_sessions::*;
pub(crate) use admin_dashboard::*;
pub(crate) use admin_dashboard_panels::*;
pub(crate) use admin_devices::*;
pub(crate) use admin_misc::*;
pub(crate) use admin_users::*;
pub(crate) use auth_form::*;
pub(crate) use book_tags::*;
pub(crate) use books::*;
pub use chrome::*;
pub use chrome_error::*;
pub(crate) use chrome_nav::*;
pub(crate) use device_tokens::*;
pub(crate) use devices::*;
pub(crate) use docs::*;
pub(crate) use docs_client::*;
pub(crate) use docs_render::*;
pub(crate) use docs_search::*;
pub(crate) use docs_view::*;
pub use form_fields::*;
pub use format_util::*;
pub use guards::*;
pub(crate) use home::*;
pub(crate) use home_view::*;
pub(crate) use landing::*;
pub(crate) use library::*;
pub(crate) use library_controls::*;
pub(crate) use library_view::*;
pub(crate) use login::*;
pub(crate) use notify::*;
pub(crate) use org_manage::*;
pub(crate) use org_manage_actions::*;
pub(crate) use org_manage_perms::*;
pub(crate) use org_manage_perms_view::*;
pub(crate) use org_manage_view::*;
pub(crate) use organization_admin::*;
pub(crate) use organization_admin_view::*;
pub(crate) use organization_books::*;
pub(crate) use organizations::*;
pub(crate) use passkeys_api::*;
pub(crate) use passkeys_ui::*;
pub use request_util::*;
pub use session::*;
pub(crate) use shares::*;
pub(crate) use shares_actions::*;
pub(crate) use signup::*;
pub(crate) use style::*;

pub(crate) const SESSION_COOKIE: &str = "hygg_session";
pub(crate) const SESSION_TTL_MS: i64 = 24 * 60 * 60 * 1000;
pub(crate) const RECOVERY_TTL_MS: i64 = 30 * 60 * 1000;
pub(crate) const DASHBOARD_RANGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
pub(crate) const LOGIN_IDENTIFIER_WINDOW_MS: i64 = 60_000;
pub(crate) const LOGIN_IDENTIFIER_LIMIT: usize = 8;

pub fn router() -> Router<AppState> {
  Router::new()
    // Public documentation center. `/docs/search` and `/docs/search.json`
    // (the typeahead's JSON feed) are static siblings of the `/docs/{slug}`
    // param route; the router prefers the static match.
    .route("/docs", get(docs_index))
    .route("/docs/search", get(docs_search))
    .route("/docs/search.json", get(docs_search_json))
    .route("/docs/{slug}", get(docs_page))
    .route("/app/home", get(home_page))
    .route("/app/home/library", get(library_fragment))
    .route("/app/shares", get(shares_page).post(share_create_post))
    .route("/app/shares/{id}/accept", post(share_accept_post))
    .route("/app/shares/{id}/decline", post(share_decline_post))
    .route("/app/shares/{id}/revoke", post(share_revoke_post))
    .route("/app/notifications/{id}/dismiss", post(notification_dismiss_post))
    .route("/login", get(login_page).post(login_post))
    .route("/signup", get(signup_page).post(signup_post))
    .route("/logout", post(logout_post))
    .route("/account", get(account_page))
    .route("/account/password", post(account_password_post))
    .route("/account/encryption", post(account_encryption_post))
    .route("/account/passkeys", get(account_passkeys_page))
    .route("/account/sessions", get(account_sessions_page))
    .route(
      "/account/sessions/revoke-all",
      post(account_sessions_revoke_all_post),
    )
    .route(
      "/account/sessions/{session_id}/revoke",
      post(account_session_revoke_post),
    )
    .route("/webauthn/register/start", post(passkey_register_start))
    .route("/webauthn/register/finish", post(passkey_register_finish))
    .route("/webauthn/auth/start", post(passkey_auth_start))
    .route("/webauthn/auth/finish", post(passkey_auth_finish))
    .route("/app/devices", get(devices_page).post(device_create_post))
    .route(
      "/app/devices/{id}/permissions",
      get(device_permissions_page).post(device_permissions_post),
    )
    .route("/app/devices/{id}/revoke", post(device_revoke_post))
    .route("/app/organizations", get(organizations_manage_index))
    .route("/app/organizations/{id}", get(organization_manage_page))
    .route(
      "/app/organizations/{id}/default-access",
      post(org_default_access_post),
    )
    .route(
      "/app/organizations/{id}/directories",
      post(org_directory_create_post),
    )
    .route(
      "/app/organizations/{id}/documents/{content_hash}/directory",
      post(org_document_directory_post),
    )
    .route("/app/organizations/{id}/groups", post(org_group_create_post))
    .route(
      "/app/organizations/{id}/groups/{group_id}/members",
      post(org_group_member_add_post),
    )
    .route(
      "/app/organizations/{id}/groups/{group_id}/members/{user_id}/remove",
      post(org_group_member_remove_post),
    )
    .route("/app/organizations/{id}/permissions", post(org_permission_set_post))
    .route(
      "/app/organizations/{id}/permissions/remove",
      post(org_permission_remove_post),
    )
    .route(
      "/app/admin/organizations",
      get(organizations_page).post(organization_create_post),
    )
    .route(
      "/app/admin/organizations/{id}",
      get(organization_page).post(organization_settings_post),
    )
    .route(
      "/app/admin/organizations/{id}/delete",
      post(organization_delete_post),
    )
    .route(
      "/app/admin/organizations/{id}/members",
      post(organization_member_post),
    )
    .route(
      "/app/admin/organizations/{id}/members/{user_id}/role",
      post(organization_member_role_post),
    )
    .route(
      "/app/admin/organizations/{id}/members/{user_id}/remove",
      post(organization_member_remove_post),
    )
    .route(
      "/app/books/{content_hash}/organization",
      post(book_organization_post),
    )
    .route("/app/books/{content_hash}/unshare", post(share_leave_post))
    .route("/app/books/{content_hash}/sync-mode", post(book_sync_mode_post))
    .route("/app/books/{content_hash}/blob/delete", post(book_blob_delete_post))
    .route("/app/books/{content_hash}/delete", post(book_delete_post))
    .route("/app/books/{content_hash}/tags", post(book_tag_add_post))
    .route("/app/books/{content_hash}/tags/remove", post(book_tag_remove_post))
    .route("/app/admin/dashboard", get(admin_dashboard_page))
    .route(
      "/app/admin/users",
      get(admin_users_page).post(admin_user_create_post),
    )
    .route("/app/admin/users/{id}/role", post(admin_user_role_post))
    .route("/app/admin/users/{id}/disabled", post(admin_user_disabled_post))
    .route("/app/admin/users/{id}/recovery", post(admin_recovery_post))
    .route("/app/admin/users/{id}/passkeys", get(admin_passkeys_page))
    .route("/app/admin/users/{id}/sessions", get(admin_sessions_page))
    .route(
      "/app/admin/users/{id}/sessions/revoke-all",
      post(admin_sessions_revoke_all_post),
    )
    .route(
      "/app/admin/users/{id}/sessions/{session_id}/revoke",
      post(admin_session_revoke_post),
    )
    .route(
      "/app/admin/users/{id}/passkeys/{passkey_id}/revoke",
      post(admin_passkey_revoke_post),
    )
    .route(
      "/app/admin/devices",
      get(admin_devices_page).post(admin_device_create_post),
    )
    .route(
      "/app/admin/devices/{id}/permissions",
      get(admin_device_permissions_page).post(admin_device_permissions_post),
    )
    .route("/app/admin/devices/{id}/revoke", post(admin_device_revoke_post))
    .route("/app/admin/devices/{id}/token", post(admin_device_token_post))
}
