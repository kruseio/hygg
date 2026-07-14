//! HTTP/web surface integration tests: HTML pages, auth/session flows, admin
//! console, organizations and the reader home. (A deployment's own pages are
//! covered by whatever injects them.)

mod admin;
mod auth;
mod books;
mod cors;
mod devices;
mod docs;
mod helpers;
mod landing;
mod library;
mod org_manage;
mod organizations;
mod passkeys;
mod sessions;
mod shares;
mod shares_common;
mod shares_progress;
