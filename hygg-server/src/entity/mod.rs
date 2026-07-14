//! The tables, as Rust types.
//!
//! Generated from the schema by `sea-orm-cli`, then corrected: SQLite reports
//! every INTEGER alike, so the generator guessed `i32` for columns holding
//! epoch milliseconds — the very width that would overflow — and marked the
//! TEXT primary keys nullable because SQLite permits it. Neither is true of
//! this schema.
//!
//! Regenerating wholesale will reintroduce both; prefer editing in place.

pub mod prelude;

pub mod api_tokens;
pub mod applied_ops;
pub mod audit_log;
pub mod book_blobs;
pub mod book_extractions;
pub mod book_tags;
pub mod bookmarks;
pub mod books;
pub mod device_book_scopes;
pub mod devices;
pub mod directories;
pub mod document_permissions;
pub mod document_shares;
pub mod highlights;
pub mod notes;
pub mod notifications;
pub mod org_group_members;
pub mod org_groups;
pub mod organization_members;
pub mod organizations;
pub mod passkeys;
pub mod progress;
pub mod reading_days;
pub mod reading_time;
pub mod recovery_codes;
pub mod sessions;
pub mod tags;
pub mod tenants;
pub mod users;
