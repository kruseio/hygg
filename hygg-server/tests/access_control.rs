//! DB-backed checks that the document permission model gates organization
//! documents: org default, per-document grants, and directory/group
//! inheritance, with owners/admins privileged and non-members denied. Personal
//! documents stay owner-only for the library but always allow the user's own
//! annotations.

use hygg_server::auth::AccessLevel;
use hygg_server::db::Db;
use hygg_server::entity::tenants;
use hygg_server::ext::NoopEntitlements;
use hygg_server::repo;
use sea_orm::*;

async fn migrated_db() -> (tempfile::TempDir, Db) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("acl.db").display());
  let db = Db::connect(&url).await.expect("connect");
  db.migrate().await.expect("migrate");
  tenants::ActiveModel {
    id: Set("t".to_owned()),
    slug: Set("acme".to_owned()),
    name: Set("A".to_owned()),
    disabled: Set(0),
    created_at: Set(0),
  }
  .insert(&db.conn)
  .await
  .unwrap();
  (dir, db)
}

async fn user(db: &Db, email: &str) -> String {
  repo::users::insert(&db.conn, "t", email, "Name", None, "user").await.unwrap()
}

async fn lib(
  db: &Db,
  user_id: &str,
  is_admin: bool,
  hash: &str,
) -> AccessLevel {
  repo::access::library_for_hash(
    &db.conn,
    &NoopEntitlements,
    "t",
    user_id,
    is_admin,
    true,
    None,
    hash,
  )
  .await
  .unwrap()
}

#[tokio::test]
async fn org_document_access_resolves_through_the_permission_model() {
  let (_dir, db) = migrated_db().await;
  let pool = &db.conn;
  let owner = user(&db, "owner@x.test").await;
  let member = user(&db, "member@x.test").await;
  let outsider = user(&db, "out@x.test").await;
  let org =
    repo::organizations::create(pool, "t", "Team", &owner).await.unwrap();
  repo::organizations::add_member(pool, "t", &org, &member, "member")
    .await
    .unwrap();
  repo::books::upsert(
    pool,
    "t",
    &owner,
    &repo::books::BookInput {
      content_hash: "hash1",
      title: "Doc",
      author: "",
      format: "txt",
      size_bytes: 10,
    },
  )
  .await
  .unwrap();
  repo::books::move_to_organization(pool, "t", &owner, "hash1", Some(&org))
    .await
    .unwrap();

  // Default permission is read_write: members get read/write, the owner is
  // privileged, a non-member sees nothing, and a tenant admin always passes.
  assert_eq!(lib(&db, &member, false, "hash1").await, AccessLevel::ReadWrite);
  assert_eq!(lib(&db, &owner, false, "hash1").await, AccessLevel::ReadWrite);
  assert_eq!(lib(&db, &outsider, false, "hash1").await, AccessLevel::None);
  assert_eq!(lib(&db, &outsider, true, "hash1").await, AccessLevel::ReadWrite);

  // Tightening the org default to none revokes the member.
  repo::organizations::set_default_access(pool, "t", &org, "none")
    .await
    .unwrap();
  assert_eq!(lib(&db, &member, false, "hash1").await, AccessLevel::None);

  // A per-document user grant restores read-only access.
  repo::permissions::set(
    pool, "t", &org, "user", &member, "document", "hash1", "read",
  )
  .await
  .unwrap();
  assert_eq!(lib(&db, &member, false, "hash1").await, AccessLevel::Read);

  // Directory + group inheritance: with the document grant removed, a group
  // grant on the document's directory grants read/write to a group member.
  repo::permissions::remove(
    pool, "t", &org, "user", &member, "document", "hash1",
  )
  .await
  .unwrap();
  let dir =
    repo::directories::create(pool, "t", &org, None, "Folder").await.unwrap();
  repo::books::set_directory(pool, "t", "hash1", Some(&dir)).await.unwrap();
  let group = repo::groups::create(pool, "t", &org, "Editors").await.unwrap();
  repo::groups::add_member(pool, "t", &group, &member).await.unwrap();
  repo::permissions::set(
    pool,
    "t",
    &org,
    "group",
    &group,
    "directory",
    &dir,
    "read_write",
  )
  .await
  .unwrap();
  assert_eq!(lib(&db, &member, false, "hash1").await, AccessLevel::ReadWrite);

  // Annotation sync follows read access for org documents...
  repo::permissions::set(
    pool,
    "t",
    &org,
    "group",
    &group,
    "directory",
    &dir,
    "none",
  )
  .await
  .unwrap();
  assert!(
    !repo::access::annotation_readable_for_hash(
      pool,
      &NoopEntitlements,
      "t",
      &member,
      false,
      true,
      None,
      "hash1"
    )
    .await
    .unwrap()
  );

  // ...but a personal document always allows the user's own annotations.
  repo::books::upsert(
    pool,
    "t",
    &owner,
    &repo::books::BookInput {
      content_hash: "personal",
      title: "Mine",
      author: "",
      format: "txt",
      size_bytes: 1,
    },
  )
  .await
  .unwrap();
  assert!(
    repo::access::annotation_readable_for_hash(
      pool,
      &NoopEntitlements,
      "t",
      &outsider,
      false,
      true,
      None,
      "personal"
    )
    .await
    .unwrap()
  );
  // The library, however, keeps personal documents owner-only.
  assert_eq!(lib(&db, &outsider, false, "personal").await, AccessLevel::None);
  assert_eq!(lib(&db, &owner, false, "personal").await, AccessLevel::ReadWrite);
}
