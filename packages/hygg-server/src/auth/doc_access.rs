//! Pure resolver for a member's effective access to an organization document.
//!
//! Precedence (most specific first): a grant targeting the document itself
//! beats a grant on a directory; a directory grant on a nearer ancestor beats a
//! farther one; at any single target a user grant beats group grants. When only
//! group grants apply at the winning target, ties resolve to the **most
//! permissive** grant. Owners/admins are privileged and always get read/write.
//! Falls back to the organization's default permission.

use crate::auth::AccessLevel;

/// One permission grant already filtered to the resolving user (their own
/// grants plus those of groups they belong to).
#[derive(Clone, Debug)]
pub struct Grant {
  pub subject_is_user: bool,
  pub target_is_document: bool,
  pub target_id: String,
  pub access: AccessLevel,
}

pub struct ResolveInput<'a> {
  pub privileged: bool,
  pub org_default: AccessLevel,
  pub book_hash: &'a str,
  /// The document's directory chain, nearest ancestor first.
  pub ancestor_dir_ids: &'a [String],
  pub grants: &'a [Grant],
}

pub fn resolve(input: &ResolveInput<'_>) -> AccessLevel {
  if input.privileged {
    return AccessLevel::ReadWrite;
  }
  if let Some(level) = level_for_target(input.grants, true, input.book_hash) {
    return level;
  }
  for dir_id in input.ancestor_dir_ids {
    if let Some(level) = level_for_target(input.grants, false, dir_id) {
      return level;
    }
  }
  input.org_default
}

/// The winning access for a single target: a user grant if present (it is more
/// specific than any group grant), otherwise the most permissive group grant.
/// `None` means no grant targets it, so resolution falls through.
fn level_for_target(
  grants: &[Grant],
  target_is_document: bool,
  target_id: &str,
) -> Option<AccessLevel> {
  let mut user: Option<AccessLevel> = None;
  let mut group: Option<AccessLevel> = None;
  for grant in grants {
    if grant.target_is_document != target_is_document
      || grant.target_id != target_id
    {
      continue;
    }
    if grant.subject_is_user {
      user = Some(more_permissive(user, grant.access));
    } else {
      group = Some(more_permissive(group, grant.access));
    }
  }
  user.or(group)
}

fn more_permissive(
  current: Option<AccessLevel>,
  candidate: AccessLevel,
) -> AccessLevel {
  match current {
    Some(existing) if existing.rank() >= candidate.rank() => existing,
    _ => candidate,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn grant(
    subject_is_user: bool,
    target_is_document: bool,
    target_id: &str,
    access: AccessLevel,
  ) -> Grant {
    Grant {
      subject_is_user,
      target_is_document,
      target_id: target_id.to_string(),
      access,
    }
  }

  fn resolve_with(
    privileged: bool,
    org_default: AccessLevel,
    ancestors: &[&str],
    grants: Vec<Grant>,
  ) -> AccessLevel {
    let ancestors: Vec<String> =
      ancestors.iter().map(|s| s.to_string()).collect();
    resolve(&ResolveInput {
      privileged,
      org_default,
      book_hash: "book",
      ancestor_dir_ids: &ancestors,
      grants: &grants,
    })
  }

  #[test]
  fn privileged_is_always_read_write() {
    let got = resolve_with(true, AccessLevel::None, &[], vec![]);
    assert_eq!(got, AccessLevel::ReadWrite);
  }

  #[test]
  fn falls_back_to_org_default_without_grants() {
    let got = resolve_with(false, AccessLevel::Read, &["dir1"], vec![]);
    assert_eq!(got, AccessLevel::Read);
  }

  #[test]
  fn document_user_grant_overrides_everything_below() {
    let grants = vec![
      grant(true, true, "book", AccessLevel::None),
      grant(false, true, "book", AccessLevel::ReadWrite),
      grant(true, false, "dir1", AccessLevel::ReadWrite),
    ];
    // Explicit user grant on the document revokes despite permissive group +
    // directory grants and a permissive org default.
    let got = resolve_with(false, AccessLevel::ReadWrite, &["dir1"], grants);
    assert_eq!(got, AccessLevel::None);
  }

  #[test]
  fn group_grants_on_document_take_the_most_permissive() {
    let grants = vec![
      grant(false, true, "book", AccessLevel::Read),
      grant(false, true, "book", AccessLevel::ReadWrite),
    ];
    let got = resolve_with(false, AccessLevel::None, &[], grants);
    assert_eq!(got, AccessLevel::ReadWrite);
  }

  #[test]
  fn document_group_grant_beats_directory_user_grant() {
    let grants = vec![
      grant(false, true, "book", AccessLevel::Read),
      grant(true, false, "dir1", AccessLevel::ReadWrite),
    ];
    let got = resolve_with(false, AccessLevel::ReadWrite, &["dir1"], grants);
    assert_eq!(got, AccessLevel::Read);
  }

  #[test]
  fn nearest_directory_wins_over_farther_ancestor() {
    let grants = vec![
      grant(false, false, "near", AccessLevel::Read),
      grant(false, false, "far", AccessLevel::ReadWrite),
    ];
    let got = resolve_with(false, AccessLevel::None, &["near", "far"], grants);
    assert_eq!(got, AccessLevel::Read);
  }

  #[test]
  fn directory_grant_inherits_to_nested_document() {
    let grants = vec![grant(false, false, "far", AccessLevel::ReadWrite)];
    let got =
      resolve_with(false, AccessLevel::None, &["near", "mid", "far"], grants);
    assert_eq!(got, AccessLevel::ReadWrite);
  }

  #[test]
  fn user_beats_group_at_the_same_directory() {
    let grants = vec![
      grant(true, false, "dir1", AccessLevel::Read),
      grant(false, false, "dir1", AccessLevel::ReadWrite),
    ];
    let got = resolve_with(false, AccessLevel::None, &["dir1"], grants);
    assert_eq!(got, AccessLevel::Read);
  }
}
