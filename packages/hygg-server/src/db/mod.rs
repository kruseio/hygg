//! Database access: one SQLite connection, and the migrations that shape it.
//!
//! SQLite is the only backend. The server is a single binary you point at a
//! file — there is no cluster to talk to and nothing that wants a second
//! database engine in the way. The ORM is not tied to SQLite, so nothing here
//! forecloses another one later; it is simply not carried until something
//! needs it.

use std::path::Path;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;

use crate::migration::{Migrator, SchemaExt};

#[derive(Clone)]
pub struct Db {
  pub conn: DatabaseConnection,
}

impl Db {
  /// Open the database at `database_url`, creating the file and its parent
  /// directory if they are not there yet.
  pub async fn connect(database_url: &str) -> Result<Self, DbErr> {
    let url = prepare_sqlite_url(database_url)?;
    let mut options = ConnectOptions::new(url);
    // An in-memory database lives inside its connection: a second one would be
    // a different, empty database, so the pool has to stay at one.
    options.max_connections(if is_sqlite_memory(database_url) { 1 } else { 5 });
    options.sqlx_logging(false);
    let conn = Database::connect(options).await?;
    Ok(Self { conn })
  }

  /// Bring the schema up to date.
  pub async fn migrate(&self) -> Result<(), DbErr> {
    Migrator::up(&self.conn, None).await
  }

  /// Bring the schema up to date, including an embedder's own tables. Theirs
  /// run after the core's.
  pub async fn migrate_with(&self, ext: SchemaExt) -> Result<(), DbErr> {
    ext.install();
    Migrator::up(&self.conn, None).await
  }
}

fn is_sqlite_memory(database_url: &str) -> bool {
  database_url.contains(":memory:")
}

/// Ensure a SQLite file URL is openable: create the parent directory and ask
/// for `mode=rwc` so the file is created on demand. In-memory URLs pass
/// through untouched.
fn prepare_sqlite_url(database_url: &str) -> Result<String, DbErr> {
  if is_sqlite_memory(database_url) {
    return Ok(database_url.to_string());
  }
  let without_scheme =
    database_url.strip_prefix("sqlite://").unwrap_or(database_url);
  let path_part = without_scheme.split('?').next().unwrap_or(without_scheme);
  if let Some(parent) = Path::new(path_part).parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent).map_err(|e| {
      DbErr::Conn(sea_orm::RuntimeErr::Internal(format!(
        "cannot create sqlite dir {parent:?}: {e}"
      )))
    })?;
  }
  if database_url.contains("mode=") {
    Ok(database_url.to_string())
  } else if database_url.contains('?') {
    Ok(format!("{database_url}&mode=rwc"))
  } else {
    Ok(format!("{database_url}?mode=rwc"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sqlite_url_gets_rwc_mode() {
    // A parent-less filename so the test never creates a stray `data/` dir in
    // the crate root (prepare_sqlite_url only `create_dir_all`s a real parent).
    let url = prepare_sqlite_url("sqlite://hygg-server.db").unwrap();
    assert!(url.contains("mode=rwc"));
  }

  #[test]
  fn an_existing_mode_is_left_alone() {
    let url = prepare_sqlite_url("sqlite://x.db?mode=ro").unwrap();
    assert_eq!(url, "sqlite://x.db?mode=ro");
  }

  #[test]
  fn an_in_memory_url_passes_through() {
    assert_eq!(
      prepare_sqlite_url("sqlite::memory:").unwrap(),
      "sqlite::memory:"
    );
  }
}
