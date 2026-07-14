//! Password hashing with Argon2id. Passwords (and recovery tokens) are
//! human-chosen / lower-entropy, so they use a slow KDF — unlike API tokens,
//! which are high-entropy random and only SHA-256 hashed (see `token`).

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{
  PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};

/// Hash a password into a PHC string suitable for storage.
pub fn hash_password(
  password: &str,
) -> Result<String, argon2::password_hash::Error> {
  let salt = SaltString::generate(&mut OsRng);
  let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
  Ok(hash.to_string())
}

/// Verify a password against a stored PHC string. Returns false on any
/// mismatch or malformed hash (never panics).
pub fn verify_password(password: &str, phc: &str) -> bool {
  let Ok(parsed) = PasswordHash::new(phc) else {
    return false;
  };
  Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hash_then_verify_roundtrips() {
    let phc = hash_password("correct horse battery staple").unwrap();
    assert!(verify_password("correct horse battery staple", &phc));
    assert!(!verify_password("wrong password", &phc));
  }

  #[test]
  fn malformed_hash_does_not_verify() {
    assert!(!verify_password("anything", "not-a-phc-string"));
  }
}
