//! Cross-client sync: progress pushed by one device is pulled by another, with
//! op-id idempotency and last-write-wins ordering.

mod access;
mod annotations;
mod convert;
mod helpers;
mod progress;
mod reading;
