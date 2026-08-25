//! The baseline migration builds the whole schema.
//!
//! The schema used to be 22 incremental SQL files per backend; it is now one
//! Rust migration. These assert the result still has every table and that a
//! second run is a no-op.

use hygg_server::db::Db;
use hygg_server::migration::Migrator;
use sea_orm_migration::{MigratorTrait, SchemaManager};

/// Every table the server expects. If one goes missing the failure is a
/// confusing runtime error much later, so name them all here.
const TABLES: &[&str] = &[
  "api_tokens",
  "applied_ops",
  "audit_log",
  "book_blobs",
  "book_extractions",
  "book_tags",
  "bookmarks",
  "books",
  "device_book_scopes",
  "devices",
  "directories",
  "document_permissions",
  "document_shares",
  "encryption_markers",
  "highlights",
  "notes",
  "notifications",
  "org_group_members",
  "org_groups",
  "organization_members",
  "organizations",
  "passkeys",
  "progress",
  "reading_days",
  "reading_time",
  "recovery_codes",
  "sessions",
  "tags",
  "tenants",
  "users",
];

async fn migrated() -> Db {
  let db = Db::connect("sqlite::memory:").await.expect("connect");
  db.migrate().await.expect("migrate");
  db
}

#[tokio::test]
async fn the_baseline_creates_every_table() {
  let db = migrated().await;
  let manager = SchemaManager::new(&db.conn);
  for table in TABLES {
    assert!(
      manager.has_table(*table).await.expect("has_table"),
      "missing table: {table}"
    );
  }
}

#[tokio::test]
async fn migrating_an_up_to_date_database_does_nothing() {
  let db = migrated().await;
  // Booting a server against an existing database runs this every time.
  db.migrate().await.expect("second migrate is a no-op");
  let manager = SchemaManager::new(&db.conn);
  assert!(manager.has_table("users").await.expect("has_table"));
}

#[tokio::test]
async fn the_schema_is_one_migration() {
  // The 22 incremental files were squashed into a single baseline; if that
  // ever silently becomes a chain again, this catches it.
  assert_eq!(Migrator::migrations().len(), 1);
}
