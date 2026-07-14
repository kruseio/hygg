//! The authenticated caller resolved from a bearer token: who they are
//! (tenant/user/device) and what their device is allowed to do. Built by the
//! `authn` middleware and injected into handlers as an extractor.

use std::collections::HashMap;

use serde::Serialize;

/// A user's role: an administrator, or an ordinary user. Any finer distinction
/// is a deployment's own concern — resolved by the [`Entitlements`] hook and
/// carried on the [`Principal`] as `personal_sync` — so the server itself has
/// only these two.
///
/// [`Entitlements`]: crate::ext::Entitlements
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
  Admin,
  User,
}

impl Role {
  /// Parse the stored role string. Only `admin` is distinguished; every other
  /// value is an ordinary user, so a row written by an older schema still
  /// resolves sensibly.
  pub fn parse(value: &str) -> Role {
    match value {
      "admin" => Role::Admin,
      _ => Role::User,
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      Role::Admin => "admin",
      Role::User => "user",
    }
  }

  pub fn is_admin(self) -> bool {
    matches!(self, Role::Admin)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessLevel {
  ReadWrite,
  Read,
  None,
}

impl AccessLevel {
  pub fn parse(value: &str) -> Self {
    match value {
      "read" => Self::Read,
      "none" => Self::None,
      _ => Self::ReadWrite,
    }
  }

  pub fn as_str(&self) -> &'static str {
    match self {
      Self::ReadWrite => "read_write",
      Self::Read => "read",
      Self::None => "none",
    }
  }

  pub fn can_read(&self) -> bool {
    matches!(self, Self::ReadWrite | Self::Read)
  }

  pub fn can_write(&self) -> bool {
    matches!(self, Self::ReadWrite)
  }

  /// Higher rank = more permissive (`none` 0 < `read` 1 < `read_write` 2).
  pub fn rank(&self) -> u8 {
    match self {
      Self::ReadWrite => 2,
      Self::Read => 1,
      Self::None => 0,
    }
  }

  /// The more restrictive of two levels (used to combine independent gates,
  /// e.g. device scope and per-user document permission).
  pub fn min(self, other: Self) -> Self {
    if self.rank() <= other.rank() { self } else { other }
  }
}

impl Serialize for AccessLevel {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(self.as_str())
  }
}

#[derive(Clone, Debug)]
pub struct Principal {
  pub tenant_id: String,
  pub user_id: String,
  pub device_id: String,
  pub role: Role,
  /// Whether the caller may sync their own (personal, non-org) library. Set by
  /// the [`Entitlements`] hook during authentication; true unless an override
  /// says otherwise. Organization documents are covered separately by the
  /// caller's org seat.
  ///
  /// [`Entitlements`]: crate::ext::Entitlements
  pub personal_sync: bool,
  pub default_access: AccessLevel,
  pub read_only: bool,
  pub progress_sync_denied: bool,
  /// Per-book access overrides keyed by the client-visible book id.
  pub book_access: HashMap<String, AccessLevel>,
}

impl Principal {
  pub fn access_for_book(&self, book_id: &str) -> &AccessLevel {
    self.book_access.get(book_id).unwrap_or(&self.default_access)
  }

  pub fn can_read_book(&self, book_id: &str) -> bool {
    self.access_for_book(book_id).can_read()
  }

  pub fn can_write_book(&self, book_id: &str) -> bool {
    self.access_for_book(book_id).can_write()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn role_parsing_and_access() {
    assert_eq!(Role::parse("admin"), Role::Admin);
    assert_eq!(Role::parse("user"), Role::User);
    // A value written by an older schema still resolves to an ordinary user.
    assert_eq!(Role::parse("anything-else"), Role::User);
    assert!(Role::Admin.is_admin());
    assert!(!Role::User.is_admin());
    assert_eq!(Role::Admin.as_str(), "admin");
    assert_eq!(Role::User.as_str(), "user");
  }

  fn principal(
    default_access: AccessLevel,
    book_access: HashMap<String, AccessLevel>,
  ) -> Principal {
    Principal {
      tenant_id: "t".into(),
      user_id: "u".into(),
      device_id: "d".into(),
      role: Role::User,
      personal_sync: true,
      default_access,
      read_only: false,
      progress_sync_denied: false,
      book_access,
    }
  }

  #[test]
  fn read_write_default_allows_any_book() {
    let p = principal(AccessLevel::ReadWrite, HashMap::new());
    assert!(p.can_read_book("anything"));
    assert!(p.can_write_book("anything"));
  }

  #[test]
  fn overrides_win_over_device_default() {
    let overrides = HashMap::from([
      ("book-a".to_string(), AccessLevel::ReadWrite),
      ("book-b".to_string(), AccessLevel::Read),
      ("book-c".to_string(), AccessLevel::None),
    ]);
    let p = principal(AccessLevel::None, overrides);
    assert!(p.can_write_book("book-a"));
    assert!(p.can_read_book("book-b"));
    assert!(!p.can_write_book("book-b"));
    assert!(!p.can_read_book("book-c"));
    assert!(!p.can_read_book("book-d"));
  }
}
