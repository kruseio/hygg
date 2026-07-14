use hygg_server::auth::password::{hash_password, verify_password};
use hygg_server::bootstrap::{ensure_bootstrap, ensure_default_tenant};
use hygg_server::config::Config;
use hygg_server::db::Db;
use hygg_server::repo;
use hygg_server::state::AppState;

async fn state() -> (tempfile::TempDir, AppState) {
  let dir = tempfile::tempdir().unwrap();
  let url = format!("sqlite://{}", dir.path().join("bootstrap.db").display());
  let db = Db::connect(&url).await.unwrap();
  db.migrate().await.unwrap();
  (dir, AppState::new(db, Config::from_env()))
}

#[tokio::test]
async fn bootstrap_promotes_existing_user_to_admin() {
  let (_dir, state) = state().await;
  let tenant_id = ensure_default_tenant(&state).await.unwrap();
  let old_hash = hash_password("old-password").unwrap();
  let user_id = repo::users::insert(
    &state.db.conn,
    &tenant_id,
    "admin@example.test",
    "Existing",
    Some(&old_hash),
    "user",
  )
  .await
  .unwrap();
  repo::users::set_disabled(&state.db.conn, &tenant_id, &user_id, true)
    .await
    .unwrap();

  unsafe {
    std::env::set_var("ADMIN_BOOTSTRAP_EMAIL", "admin@example.test");
    std::env::set_var("ADMIN_BOOTSTRAP_PASSWORD", "new-password");
  }
  ensure_bootstrap(&state).await.unwrap();
  unsafe {
    std::env::remove_var("ADMIN_BOOTSTRAP_EMAIL");
    std::env::remove_var("ADMIN_BOOTSTRAP_PASSWORD");
  }

  let user = repo::users::find_by_email(
    &state.db.conn,
    &tenant_id,
    "admin@example.test",
  )
  .await
  .unwrap()
  .unwrap();
  assert_eq!(user.role, "admin");
  assert_eq!(user.disabled, 0);
  assert_eq!(user.password_enabled, 1);
  assert!(verify_password(
    "new-password",
    user.password_hash.as_ref().unwrap()
  ));
}
