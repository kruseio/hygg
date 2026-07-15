//! The schema, defined once in Rust.
//!
//! The tables are described with SeaORM's schema builder rather than as DDL,
//! so there is one definition and the ORM emits what the database wants. The
//! server runs on SQLite; nothing here is written to a dialect, so that is a
//! choice about what to carry rather than something baked into the schema.
//!
//! An embedder adds its own tables through [`SchemaExt`], whose migrations run
//! after the core's against the same connection. They share SeaORM's
//! `seaql_migrations` ledger, so their names must not collide with the core's
//! (prefix them).

use std::sync::OnceLock;

use sea_orm_migration::prelude::*;

// Public so an embedder's own migration can reference the core tables its
// foreign keys point at, rather than re-spelling their names as strings.
pub mod baseline;

pub use baseline::Baseline;

/// Builds an embedder's migrations. A plain `fn` pointer so it can live in a
/// static and be called from [`MigratorTrait::migrations`], which takes no
/// state of its own.
pub type MigrationsFn = fn() -> Vec<Box<dyn MigrationTrait>>;

static EXT_MIGRATIONS: OnceLock<MigrationsFn> = OnceLock::new();

/// Extra migrations layered onto the core schema.
///
/// Registered process-wide because [`MigratorTrait`] resolves its list from a
/// static method with nothing to carry an instance on.
#[derive(Clone, Copy)]
pub struct SchemaExt {
  migrations: MigrationsFn,
}

impl SchemaExt {
  /// Install an embedder's migrations. They run after the core's.
  pub fn new(migrations: MigrationsFn) -> Self {
    Self { migrations }
  }

  /// Register for [`Migrator`] to pick up. Later calls are ignored — the set
  /// is fixed for the life of the process.
  pub(crate) fn install(self) {
    let _ = EXT_MIGRATIONS.set(self.migrations);
  }
}

/// The core schema plus whatever an embedder registered.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    let mut migrations: Vec<Box<dyn MigrationTrait>> = vec![Box::new(Baseline)];
    if let Some(ext) = EXT_MIGRATIONS.get() {
      migrations.extend(ext());
    }
    migrations
  }
}
