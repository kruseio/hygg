use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::state::AppState;
use tower::ServiceExt;

async fn test_state() -> AppState {
  let db = Db::connect("sqlite::memory:").await.expect("connect sqlite memory");
  AppState::new(db, Config::from_env())
}

#[tokio::test]
async fn health_returns_ok() {
  let app = hygg_server::app(test_state().await);
  let response = app
    .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let bytes = response.into_body().collect().await.unwrap().to_bytes();
  let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
  assert_eq!(json["status"], "ok");
}
