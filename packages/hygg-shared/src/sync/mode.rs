//! How a document is synchronized. A single knob that exists at two levels: the
//! server keeps an account-wide *ceiling* per document, and each client keeps a
//! local *clamp*. The effective mode is the more restrictive of the two
//! ([`SyncMode::most_restrictive`]), so a client may lower its own copy but
//! never raise it above the server's policy.
//!
//! Ordered most- to least-permissive: `Full > Metadata > Off`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Per-document sync policy. The wire/config token is the lowercase variant
/// name (`full` | `metadata` | `off`).
#[derive(
  Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum SyncMode {
  /// Metadata record + document bytes (blob) + reading state. The default, and
  /// the historical all-in behavior.
  #[default]
  Full,
  /// Metadata record + reading state (progress, bookmarks, highlights, notes),
  /// but never the document bytes — the file stays on the device.
  Metadata,
  /// Nothing about this document leaves (or reaches) the device.
  Off,
}

impl SyncMode {
  /// Rank from most permissive (2) to least (0); used to compare and combine.
  const fn rank(self) -> u8 {
    match self {
      SyncMode::Full => 2,
      SyncMode::Metadata => 1,
      SyncMode::Off => 0,
    }
  }

  /// The more restrictive of two modes — the effective mode when a server
  /// ceiling and a client clamp both apply. Combining is a `min` on rank, so a
  /// client can lower its mode but never raise it above the server's.
  pub fn most_restrictive(self, other: SyncMode) -> SyncMode {
    if self.rank() <= other.rank() { self } else { other }
  }

  /// Whether the document's bytes (blob) may sync in this mode.
  pub fn syncs_blob(self) -> bool {
    matches!(self, SyncMode::Full)
  }

  /// Whether reading state (progress + annotations) and the metadata record may
  /// sync in this mode.
  pub fn syncs_state(self) -> bool {
    matches!(self, SyncMode::Full | SyncMode::Metadata)
  }

  /// Whether anything at all syncs. False only for [`SyncMode::Off`].
  pub fn syncs_anything(self) -> bool {
    !matches!(self, SyncMode::Off)
  }

  /// The wire / config token: `full` | `metadata` | `off`.
  pub fn as_str(self) -> &'static str {
    match self {
      SyncMode::Full => "full",
      SyncMode::Metadata => "metadata",
      SyncMode::Off => "off",
    }
  }

  /// Parse a stored/wire token, falling back to [`SyncMode::Full`] for anything
  /// unrecognized (an older row, a hand-edited value) so a bad token never
  /// silently disables sync.
  pub fn from_token_or_default(s: &str) -> SyncMode {
    s.parse().unwrap_or_default()
  }
}

impl fmt::Display for SyncMode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl FromStr for SyncMode {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.trim().to_ascii_lowercase().as_str() {
      "full" => Ok(SyncMode::Full),
      "metadata" | "meta" => Ok(SyncMode::Metadata),
      "off" | "disabled" | "none" => Ok(SyncMode::Off),
      _ => Err(()),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_is_full() {
    assert_eq!(SyncMode::default(), SyncMode::Full);
  }

  #[test]
  fn most_restrictive_takes_the_lower_rank() {
    use SyncMode::*;
    assert_eq!(Full.most_restrictive(Metadata), Metadata);
    assert_eq!(Metadata.most_restrictive(Full), Metadata);
    assert_eq!(Full.most_restrictive(Off), Off);
    assert_eq!(Off.most_restrictive(Metadata), Off);
    assert_eq!(Full.most_restrictive(Full), Full);
    // Commutative.
    assert_eq!(Metadata.most_restrictive(Off), Off.most_restrictive(Metadata));
  }

  #[test]
  fn capability_predicates() {
    assert!(SyncMode::Full.syncs_blob());
    assert!(!SyncMode::Metadata.syncs_blob());
    assert!(!SyncMode::Off.syncs_blob());

    assert!(SyncMode::Full.syncs_state());
    assert!(SyncMode::Metadata.syncs_state());
    assert!(!SyncMode::Off.syncs_state());

    assert!(SyncMode::Full.syncs_anything());
    assert!(SyncMode::Metadata.syncs_anything());
    assert!(!SyncMode::Off.syncs_anything());
  }

  #[test]
  fn parse_round_trips_and_accepts_aliases() {
    for m in [SyncMode::Full, SyncMode::Metadata, SyncMode::Off] {
      assert_eq!(m.as_str().parse::<SyncMode>(), Ok(m));
    }
    assert_eq!("meta".parse(), Ok(SyncMode::Metadata));
    assert_eq!("  OFF ".parse(), Ok(SyncMode::Off));
    assert_eq!("disabled".parse(), Ok(SyncMode::Off));
    assert_eq!(SyncMode::from_token_or_default("nonsense"), SyncMode::Full);
  }

  #[test]
  fn serde_uses_lowercase_tokens() {
    let json = serde_json::to_string(&SyncMode::Metadata).unwrap();
    assert_eq!(json, "\"metadata\"");
    let back: SyncMode = serde_json::from_str("\"off\"").unwrap();
    assert_eq!(back, SyncMode::Off);
  }
}
