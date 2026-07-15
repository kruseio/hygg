use hygg_server::bootstrap::ensure_default_tenant;
use hygg_server::repo;
use serde_json::json;

use crate::helpers::*;

#[tokio::test]
async fn reading_time_and_day_aggregate_with_last_write_wins() {
  let (_dir, state) = setup().await;
  let token = register_device(&state).await;
  let tenant_id = ensure_default_tenant(&state).await.unwrap();
  let user = repo::users::find_by_email(&state.db.conn, &tenant_id, "u@x.y")
    .await
    .unwrap()
    .unwrap();

  // Cumulative reading time + a day bucket land and apply.
  let body = json_body(
    push(
      &state,
      &token,
      json!({ "ops": [
        { "op_id": "rt1", "kind": "reading_time", "book_id": "book-1",
          "updated_at": 1000, "data": { "seconds": 600 } },
        { "op_id": "rd1", "kind": "reading_day", "book_id": "book-1",
          "updated_at": 1000, "data": { "day": "2026-06-25", "seconds": 600 } },
      ] }),
    )
    .await,
  )
  .await;
  let applied = body["applied"].as_array().unwrap();
  assert!(applied.iter().any(|v| v == "rt1"));
  assert!(applied.iter().any(|v| v == "rd1"));

  // A newer cumulative value wins; an older one does not overwrite it.
  push(
    &state,
    &token,
    json!({ "ops": [
      { "op_id": "rt2", "kind": "reading_time", "book_id": "book-1",
        "updated_at": 2000, "data": { "seconds": 900 } },
      { "op_id": "rt3", "kind": "reading_time", "book_id": "book-1",
        "updated_at": 500, "data": { "seconds": 5 } },
    ] }),
  )
  .await;

  let total =
    repo::reading::total_seconds(&state.db.conn, &tenant_id, &user.id)
      .await
      .unwrap();
  assert_eq!(total, 900, "newer cumulative wins, older is ignored");

  let by_book =
    repo::reading::seconds_by_book(&state.db.conn, &tenant_id, &user.id)
      .await
      .unwrap();
  assert_eq!(by_book, vec![("book-1".to_string(), 900)]);

  let days = repo::reading::active_days(&state.db.conn, &tenant_id, &user.id)
    .await
    .unwrap();
  assert_eq!(days, vec!["2026-06-25".to_string()]);
}
