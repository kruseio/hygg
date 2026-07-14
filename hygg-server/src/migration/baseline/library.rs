//! Documents, their bytes, cached extractions and tags.

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use super::identity::{Users, tenant_fk};

#[derive(DeriveIden)]
pub enum Books {
  Table,
  Id,
  TenantId,
  OwnerUserId,
  OrganizationId,
  DirectoryId,
  ContentHash,
  Title,
  Author,
  Format,
  FileName,
  SizeBytes,
  SyncMode,
  CreatedAt,
  UpdatedAt,
}

#[derive(DeriveIden)]
pub enum BookBlobs {
  Table,
  Id,
  TenantId,
  BookId,
  Storage,
  Bytes,
  Path,
  ByteLen,
  Sha256,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum BookExtractions {
  Table,
  Id,
  TenantId,
  ContentHash,
  ExtractorVersion,
  Col,
  Title,
  Format,
  Text,
  ByteLen,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum Tags {
  Table,
  Id,
  TenantId,
  ScopeType,
  ScopeId,
  Name,
  CreatedAt,
}

#[derive(DeriveIden)]
pub enum BookTags {
  Table,
  Id,
  TenantId,
  TagId,
  ContentHash,
  CreatedAt,
}

pub async fn up(m: &SchemaManager<'_>) -> Result<(), DbErr> {
  m.create_table(
    Table::create()
      .table(Books::Table)
      .if_not_exists()
      .col(string(Books::Id).primary_key())
      .col(string(Books::TenantId))
      .col(string(Books::OwnerUserId))
      .col(string_null(Books::OrganizationId))
      .col(string_null(Books::DirectoryId))
      .col(string(Books::ContentHash))
      .col(string(Books::Title).default(""))
      .col(string(Books::Author).default(""))
      .col(string(Books::Format).default(""))
      .col(string_null(Books::FileName))
      .col(big_integer(Books::SizeBytes).default(0))
      // The account-wide ceiling for how this document may sync.
      .col(string(Books::SyncMode).default("full"))
      .col(big_integer(Books::CreatedAt))
      .col(big_integer(Books::UpdatedAt))
      .foreign_key(&mut tenant_fk(Books::Table, Books::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(Books::Table, Books::OwnerUserId)
          .to(Users::Table, Users::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .index(
        Index::create()
          .name("uq_books_tenant_hash")
          .col(Books::TenantId)
          .col(Books::ContentHash)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(BookBlobs::Table)
      .if_not_exists()
      .col(string(BookBlobs::Id).primary_key())
      .col(string(BookBlobs::TenantId))
      .col(string(BookBlobs::BookId))
      .col(string(BookBlobs::Storage).default("fs"))
      .col(binary_null(BookBlobs::Bytes))
      .col(string_null(BookBlobs::Path))
      .col(big_integer(BookBlobs::ByteLen).default(0))
      .col(string(BookBlobs::Sha256).default(""))
      .col(big_integer(BookBlobs::CreatedAt))
      .foreign_key(&mut tenant_fk(BookBlobs::Table, BookBlobs::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(BookBlobs::Table, BookBlobs::BookId)
          .to(Books::Table, Books::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(BookExtractions::Table)
      .if_not_exists()
      .col(string(BookExtractions::Id).primary_key())
      .col(string(BookExtractions::TenantId))
      .col(string(BookExtractions::ContentHash))
      .col(big_integer(BookExtractions::ExtractorVersion))
      .col(big_integer(BookExtractions::Col))
      .col(string(BookExtractions::Title))
      .col(string(BookExtractions::Format))
      .col(text(BookExtractions::Text))
      .col(big_integer(BookExtractions::ByteLen).default(0))
      .col(big_integer(BookExtractions::CreatedAt))
      .foreign_key(&mut tenant_fk(
        BookExtractions::Table,
        BookExtractions::TenantId,
      ))
      .index(
        Index::create()
          .name("uq_extractions_key")
          .col(BookExtractions::TenantId)
          .col(BookExtractions::ContentHash)
          .col(BookExtractions::ExtractorVersion)
          .col(BookExtractions::Col)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(Tags::Table)
      .if_not_exists()
      .col(string(Tags::Id).primary_key())
      .col(string(Tags::TenantId))
      .col(string(Tags::ScopeType))
      .col(string(Tags::ScopeId))
      .col(string(Tags::Name))
      .col(big_integer(Tags::CreatedAt))
      .foreign_key(&mut tenant_fk(Tags::Table, Tags::TenantId))
      .index(
        Index::create()
          .name("uq_tags_scope_name")
          .col(Tags::TenantId)
          .col(Tags::ScopeType)
          .col(Tags::ScopeId)
          .col(Tags::Name)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  m.create_table(
    Table::create()
      .table(BookTags::Table)
      .if_not_exists()
      .col(string(BookTags::Id).primary_key())
      .col(string(BookTags::TenantId))
      .col(string(BookTags::TagId))
      .col(string(BookTags::ContentHash))
      .col(big_integer(BookTags::CreatedAt))
      .foreign_key(&mut tenant_fk(BookTags::Table, BookTags::TenantId))
      .foreign_key(
        ForeignKey::create()
          .from(BookTags::Table, BookTags::TagId)
          .to(Tags::Table, Tags::Id)
          .on_delete(ForeignKeyAction::Cascade),
      )
      .index(
        Index::create()
          .name("uq_book_tags")
          .col(BookTags::TenantId)
          .col(BookTags::TagId)
          .col(BookTags::ContentHash)
          .unique(),
      )
      .to_owned(),
  )
  .await?;

  for (name, table, cols) in [
    (
      "idx_books_owner",
      Books::Table,
      vec![Books::TenantId, Books::OwnerUserId],
    ),
    (
      "idx_books_org",
      Books::Table,
      vec![Books::TenantId, Books::OrganizationId],
    ),
  ] {
    let mut idx = Index::create();
    idx.name(name).table(table).if_not_exists();
    for c in cols {
      idx.col(c);
    }
    m.create_index(idx.to_owned()).await?;
  }

  m.create_index(
    Index::create()
      .name("idx_extractions_lookup")
      .table(BookExtractions::Table)
      .if_not_exists()
      .col(BookExtractions::TenantId)
      .col(BookExtractions::ContentHash)
      .to_owned(),
  )
  .await?;

  for (name, cols) in
    [("idx_tags_scope", vec![Tags::TenantId, Tags::ScopeType, Tags::ScopeId])]
  {
    let mut idx = Index::create();
    idx.name(name).table(Tags::Table).if_not_exists();
    for c in cols {
      idx.col(c);
    }
    m.create_index(idx.to_owned()).await?;
  }

  for (name, cols) in [
    ("idx_book_tags_hash", vec![BookTags::TenantId, BookTags::ContentHash]),
    ("idx_book_tags_tag", vec![BookTags::TenantId, BookTags::TagId]),
  ] {
    let mut idx = Index::create();
    idx.name(name).table(BookTags::Table).if_not_exists();
    for c in cols {
      idx.col(c);
    }
    m.create_index(idx.to_owned()).await?;
  }

  Ok(())
}
