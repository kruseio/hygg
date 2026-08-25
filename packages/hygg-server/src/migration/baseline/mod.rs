//! The whole schema as one migration.
//!
//! This replaces the 22 incremental SQL migrations the server carried before
//! its first release. Nothing had shipped, so there was no history worth
//! keeping — and a baseline states the schema as it is rather than as a
//! sequence of edits that reached it. Two things that could only be described
//! as scars are gone with it: columns bolted on by `ALTER TABLE`, and a
//! `users.role` default of `nonpaying` that SQLite could not drop without
//! rebuilding a table sixteen foreign keys point at.
//!
//! Split into modules by domain purely to keep each file within the LOC
//! budget; they are one migration and apply together.

use sea_orm_migration::prelude::*;

pub mod annotations;
pub mod credentials;
pub mod encryption;
pub mod identity;
pub mod library;
pub mod misc;
pub mod orgs;
pub mod reading;
pub mod shares;

pub struct Baseline;

// Named explicitly rather than by `DeriveMigrationName`, which takes the file
// name — this lives in `baseline/mod.rs`, so it would be recorded as "mod".
// The ledger is shared with whatever an embedder adds, so the name has to say
// what it is and be unlikely to collide.
impl MigrationName for Baseline {
  fn name(&self) -> &str {
    "m20260714_000001_hygg_server_baseline"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Baseline {
  async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
    // Order matters: every table's foreign keys must already exist.
    identity::up(m).await?;
    credentials::up(m).await?;
    orgs::up(m).await?;
    shares::up(m).await?;
    library::up(m).await?;
    annotations::up(m).await?;
    reading::up(m).await?;
    encryption::up(m).await?;
    misc::up(m).await?;
    Ok(())
  }

  async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
    // A baseline has nothing to roll back to.
    Err(DbErr::Custom("the baseline schema cannot be reverted".to_owned()))
  }
}
