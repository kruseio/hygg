use uuid::Uuid;

/// Generate a new primary-key id (UUIDv4 as a lowercase hyphenated string).
/// Stored as TEXT.
pub fn new_id() -> String {
  Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ids_are_unique_and_well_formed() {
    let a = new_id();
    let b = new_id();
    assert_ne!(a, b);
    assert_eq!(a.len(), 36);
    assert_eq!(a.matches('-').count(), 4);
  }
}
