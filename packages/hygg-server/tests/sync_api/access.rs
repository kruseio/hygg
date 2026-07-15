use axum::http::StatusCode;
use hygg_server::entity::{device_book_scopes, devices, tenants};
use sea_orm::sea_query::Expr;
use sea_orm::*;
use serde_json::json;

use crate::helpers::*;

#[tokio::test]
async fn scoped_device_is_limited_to_its_books() {
  let (_dir, state) = setup().await;
  // Register the device to be scoped first, so we can find its id.
  let scoped = register_device(&state).await;
  let device_id =
    devices::Entity::find().one(&state.db.conn).await.unwrap().unwrap().id;
  let tenant_id =
    tenants::Entity::find().one(&state.db.conn).await.unwrap().unwrap().id;
  // Deny by default, then allow exactly one book.
  devices::Entity::update_many()
    .col_expr(devices::Column::DefaultAccess, Expr::value("none"))
    .exec(&state.db.conn)
    .await
    .unwrap();
  device_book_scopes::ActiveModel {
    id: Set("scope-1".to_owned()),
    tenant_id: Set(tenant_id.clone()),
    device_id: Set(device_id.clone()),
    book_id: Set("book-allowed".to_owned()),
    access: Set("read_write".to_owned()),
  }
  .insert(&state.db.conn)
  .await
  .unwrap();

  // An unrestricted device writes progress for two different books.
  let other = register_device(&state).await;
  push(
    &state,
    &other,
    json!({ "ops": [
    progress_op_for("a1", "book-allowed", 7, 1000),
    progress_op_for("d1", "book-denied", 9, 1000),
  ] }),
  )
  .await;

  // The scoped device only pulls its in-scope book.
  let pulled = pull(&state, &scoped, 0).await;
  let progress = pulled["progress"].as_array().unwrap();
  assert_eq!(progress.len(), 1);
  assert_eq!(progress[0]["book_id"], "book-allowed");

  // And it cannot push to a book outside its scope (op is skipped).
  let resp = push(
    &state,
    &scoped,
    json!({ "ops": [progress_op_for("s1", "book-denied", 1, 2000)] }),
  )
  .await;
  let body = json_body(resp).await;
  assert_eq!(body["applied"], json!([]));
  assert_eq!(body["skipped"], json!(["s1"]));
}

#[tokio::test]
async fn read_access_device_cannot_push_annotations() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;
  // Mark the (single) device read-only directly in the DB.
  devices::Entity::update_many()
    .col_expr(devices::Column::DefaultAccess, Expr::value("read"))
    .col_expr(devices::Column::ReadOnly, Expr::value(1))
    .col_expr(devices::Column::ProgressSyncDenied, Expr::value(1))
    .exec(&state.db.conn)
    .await
    .unwrap();

  let op = json!({
    "op_id": "bm-ro", "kind": "bookmark", "book_id": "book-1",
    "updated_at": 1000, "data": { "mark": "a", "line": 1 }
  });
  let resp = push(&state, &token, json!({ "ops": [op] })).await;
  assert_eq!(resp.status(), StatusCode::OK);
  let body = json_body(resp).await;
  assert_eq!(body["applied"], json!([]));
  assert_eq!(body["skipped"], json!(["bm-ro"]));
}
