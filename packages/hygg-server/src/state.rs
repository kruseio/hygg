//! Shared application state handed to every handler via axum's `State`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::events::EventHub;
use crate::ext::{Entitlements, NoopEntitlements, NoopWebExt, WebExt};
use crate::migration::SchemaExt;
use tokio::sync::Mutex;
use url::Url;
use webauthn_rs::prelude::{
  PasskeyAuthentication, PasskeyRegistration, Webauthn, WebauthnBuilder,
};

pub struct PendingPasskeyRegistration {
  pub tenant_id: String,
  pub user_id: String,
  pub label: String,
  pub state: PasskeyRegistration,
}

pub struct PendingPasskeyAuthentication {
  pub tenant_id: String,
  pub user_id: String,
  pub state: PasskeyAuthentication,
}

#[derive(Clone)]
pub struct AppState {
  pub db: Db,
  pub config: Config,
  /// Pub/sub hub for SSE push notifications.
  pub events: EventHub,
  pub webauthn: Arc<Webauthn>,
  pub passkey_registrations:
    Arc<Mutex<HashMap<String, PendingPasskeyRegistration>>>,
  pub passkey_authentications:
    Arc<Mutex<HashMap<String, PendingPasskeyAuthentication>>>,
  pub login_identifier_attempts: Arc<Mutex<HashMap<String, Vec<i64>>>>,
  /// Recent failed API-token / device-registration attempts, keyed by client
  /// IP, for the auth rate limiter (blocks credential spraying).
  pub api_auth_failures: Arc<Mutex<HashMap<String, Vec<i64>>>>,
  /// The access interface. Defaults to [`NoopEntitlements`] (everything
  /// granted); an override swaps in its own via
  /// [`AppState::with_entitlements`].
  pub entitlements: Arc<dyn Entitlements>,
  /// The web-UI injection interface. Defaults to [`NoopWebExt`] (nothing
  /// injected); an override adds its nav links / panels via
  /// [`AppState::with_web_ext`].
  pub web_ext: Arc<dyn WebExt>,
  /// Extension migrations run after the core's by [`runtime::serve_router`]
  /// (`None` on self-host). Installed via [`AppState::with_schema_ext`].
  ///
  /// [`runtime::serve_router`]: crate::runtime::serve_router
  pub schema_ext: Option<Arc<SchemaExt>>,
}

impl AppState {
  pub fn new(db: Db, config: Config) -> Self {
    let webauthn = build_webauthn(&config);
    Self {
      db,
      config,
      events: EventHub::default(),
      webauthn: Arc::new(webauthn),
      passkey_registrations: Arc::new(Mutex::new(HashMap::new())),
      passkey_authentications: Arc::new(Mutex::new(HashMap::new())),
      login_identifier_attempts: Arc::new(Mutex::new(HashMap::new())),
      api_auth_failures: Arc::new(Mutex::new(HashMap::new())),
      entitlements: Arc::new(NoopEntitlements),
      web_ext: Arc::new(NoopWebExt),
      schema_ext: None,
    }
  }

  /// Replace the entitlements provider. An override calls this to decide for
  /// itself who may do what.
  #[must_use]
  pub fn with_entitlements(
    mut self,
    entitlements: Arc<dyn Entitlements>,
  ) -> Self {
    self.entitlements = entitlements;
    self
  }

  /// Replace the web-UI extension (nav links, injected panels, extra styles).
  /// An override calls this to surface its own pages inside the core chrome.
  #[must_use]
  pub fn with_web_ext(mut self, web_ext: Arc<dyn WebExt>) -> Self {
    // Both are installed process-wide: `page` renders signed-out pages with no
    // state in scope, and they need the extension's styling and nav as much as
    // the signed-in ones do.
    crate::web::set_extra_css(web_ext.extra_css());
    crate::web::set_nav_groups(web_ext.as_ref());
    self.web_ext = web_ext;
    self
  }

  /// Install extension migrations to run after the core's (same pool).
  #[must_use]
  pub fn with_schema_ext(mut self, schema_ext: Arc<SchemaExt>) -> Self {
    self.schema_ext = Some(schema_ext);
    self
  }
}

fn build_webauthn(config: &Config) -> Webauthn {
  if let Ok(rp_origin) = Url::parse(&config.rp_origin)
    && let Ok(builder) = WebauthnBuilder::new(&config.rp_id, &rp_origin)
    && let Ok(webauthn) = builder.rp_name(&config.rp_name).build()
  {
    return webauthn;
  }

  tracing::warn!(
    rp_id = %config.rp_id,
    rp_origin = %config.rp_origin,
    "invalid WebAuthn RP config; passkeys will use localhost defaults"
  );
  let fallback_origin =
    Url::parse("http://localhost:3032").expect("static RP origin is valid");
  WebauthnBuilder::new("localhost", &fallback_origin)
    .expect("static RP id matches static origin")
    .rp_name("hygg")
    .build()
    .expect("static WebAuthn config is valid")
}
