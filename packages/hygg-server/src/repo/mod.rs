//! Data access. One function per query, built with the ORM's query builder —
//! there is no SQL in this crate, so there are no strings to interpolate a
//! value into by accident. Every tenant-scoped query takes `tenant_id` as an
//! argument so a handler cannot forget to scope it.

pub mod access;
pub mod blobs;
pub mod bookmarks;
pub mod books;
pub mod dashboard;
pub mod devices;
pub mod directories;
pub mod extractions;
pub mod groups;
pub mod highlights;
pub mod notes;
pub mod notifications;
pub mod ops;
pub mod organizations;
pub mod passkeys;
pub mod permissions;
pub mod progress;
pub mod reading;
pub mod recovery;
pub mod scopes;
pub mod sessions;
pub mod shares;
pub mod tags;
pub mod tenants;
pub mod tokens;
pub mod users;
