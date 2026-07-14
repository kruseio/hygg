//! Authentication primitives: password hashing, API-token minting/verification,
//! and the resolved [`Principal`]/[`Role`] handed to handlers.

pub mod doc_access;
pub mod password;
pub mod principal;
pub mod token;

pub use principal::{AccessLevel, Principal, Role};
