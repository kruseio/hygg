//! Process bootstrapping shared by the `hygg-server` binary and downstream
//! embedders: `.env` loading, tracing setup, database connect + migrate,
//! first-run bootstrap, and serving. The binary is a one-line call to [`run`];
//! an embedder reuses these same pieces while swapping in its own overrides
//! and routes.

use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::config::Config;
use crate::data_dir::{self, Claim};
use crate::db::Db;
use crate::state::AppState;

/// Self-host entry point: claim the data directory, load env, initialise
/// logging, open + migrate the database, run first-run bootstrap, and serve the
/// standalone app (with the default [`NoopEntitlements`]) until shutdown.
///
/// [`NoopEntitlements`]: crate::ext::NoopEntitlements
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
  load_dotenv();
  let config = Config::from_env();

  // Before anything writes: initialising logging and opening the database both
  // create files under the data directory, and the claim can only tell whether
  // the directory is ours while it is still untouched. An embedder assembling
  // its own startup owes itself the same call, in the same position.
  let claim = data_dir::claim(Path::new(&config.data_dir))?;

  // Hold the appender guard for the whole process so buffered logs are flushed.
  let _log_guard = init_tracing(&config.log_dir, "hygg-server");
  report_claim(claim, &config.data_dir);

  let db = Db::connect(&config.database_url).await?;
  tracing::info!("connected to database");
  let state = AppState::new(db, config);
  serve_state(state).await
}

/// Log what [`data_dir::claim`] did, now that there is somewhere to log it.
/// Only the ordinary restart passes in silence; the rest are each a thing an
/// operator would want to see confirmed in the first lines of a boot.
fn report_claim(claim: Claim, data_dir: &str) {
  match claim {
    Claim::Owned => {}
    Claim::Created => tracing::info!("created data directory {data_dir}"),
    Claim::ClaimedEmpty => {
      tracing::info!("claimed empty data directory {data_dir}");
    }
    Claim::ClaimedExisting => tracing::info!(
      "claimed existing data directory {data_dir} and marked it as \
       hygg-server's"
    ),
  }
}

/// Migrate, run first-run bootstrap, bind, and serve the standalone self-host
/// app assembled from `state`. The state carries whatever entitlements the
/// embedder injected.
pub async fn serve_state(
  state: AppState,
) -> Result<(), Box<dyn std::error::Error>> {
  let router = crate::app(state.clone());
  serve_router(state, router).await
}

/// Like [`serve_state`], but serves a caller-supplied `router` instead of the
/// default app. An embedder uses this to serve its composed router
/// (`layers(routes(state).merge(own_router), &config)`) in a single process,
/// after the shared migrate + bootstrap steps.
pub async fn serve_router(
  state: AppState,
  router: axum::Router,
) -> Result<(), Box<dyn std::error::Error>> {
  prepare(&state).await?;
  bind_and_serve(state, router).await
}

/// Migrate the database (core migrations plus any installed schema extension)
/// and run the idempotent first-run bootstrap. Exposed separately from
/// [`serve_router`] so an embedder can seed its own state (e.g. default plans)
/// between bootstrap and serving.
pub async fn prepare(
  state: &AppState,
) -> Result<(), Box<dyn std::error::Error>> {
  match &state.schema_ext {
    Some(ext) => state.db.migrate_with(*ext.as_ref()).await?,
    None => state.db.migrate().await?,
  }
  tracing::info!("migrations applied");
  crate::bootstrap::ensure_bootstrap(state).await?;
  Ok(())
}

/// Bind the configured address and serve `router` until shutdown.
pub async fn bind_and_serve(
  state: AppState,
  router: axum::Router,
) -> Result<(), Box<dyn std::error::Error>> {
  let bind_addr = state.config.bind_addr.clone();
  let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
  tracing::info!("listening on {bind_addr}");
  crate::serve_on(listener, router).await?;
  Ok(())
}

/// Initialise logging: always to stdout, and additionally to a daily-rotating
/// file under `<log_dir>/<service>/` retained for 30 days. Returns the
/// non-blocking writer's guard, which the caller keeps alive so buffered log
/// lines are flushed on shutdown. Falls back to stdout-only if the log
/// directory can't be created (e.g. a read-only filesystem), so logging setup
/// never prevents the server from booting.
pub fn init_tracing(log_dir: &str, service: &str) -> Option<WorkerGuard> {
  let filter = || {
    EnvFilter::try_from_default_env()
      .unwrap_or_else(|_| EnvFilter::new("hygg_server=info,tower_http=info"))
  };
  let dir = Path::new(log_dir).join(service);
  let appender = std::fs::create_dir_all(&dir).ok().and_then(|()| {
    RollingFileAppender::builder()
      .rotation(Rotation::DAILY)
      .filename_prefix(service)
      .filename_suffix("log")
      .max_log_files(30)
      .build(&dir)
      .ok()
  });
  match appender {
    Some(appender) => {
      let (file_writer, guard) = tracing_appender::non_blocking(appender);
      tracing_subscriber::registry()
        .with(filter())
        .with(fmt::layer().with_writer(std::io::stdout))
        .with(fmt::layer().with_ansi(false).with_writer(file_writer))
        .init();
      Some(guard)
    }
    None => {
      tracing_subscriber::registry()
        .with(filter())
        .with(fmt::layer().with_writer(std::io::stdout))
        .init();
      tracing::warn!(
        "could not open log directory {log_dir}; logging to stdout only"
      );
      None
    }
  }
}

/// Load server env from the current directory and, when running from the
/// workspace root, from `packages/hygg-server/.env`. Explicit environment
/// variables keep priority because dotenvy does not overwrite existing values.
pub fn load_dotenv() {
  let _ = dotenvy::dotenv();
  for path in dotenv_fallback_paths() {
    if path.is_file() {
      let _ = dotenvy::from_path(path);
    }
  }
}

fn dotenv_fallback_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();
  if let Ok(cwd) = std::env::current_dir() {
    paths.push(cwd.join("packages").join("hygg-server").join(".env"));
  }
  if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
    paths.push(Path::new(manifest_dir).join(".env"));
  }
  paths
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn dotenv_fallbacks_include_workspace_and_manifest_server_envs() {
    let paths = dotenv_fallback_paths();

    // Pinned to the full `packages/hygg-server/.env` rather than a trailing
    // `hygg-server/.env`: this crate moved under packages/ once, and an
    // `ends_with` on the last two components matches the pre-move path just as
    // happily as the current one — so it would have watched the workspace-root
    // fallback break without saying a word.
    if let Ok(cwd) = std::env::current_dir() {
      assert!(
        paths.contains(&cwd.join("packages").join("hygg-server").join(".env"))
      );
    }

    // The manifest-dir fallback is absolute and resolved at compile time.
    assert!(paths.iter().any(|p| p.ends_with("hygg-server/.env")));
  }
}
