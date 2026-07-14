use super::*;

#[derive(Deserialize)]
pub(crate) struct PasskeyRegisterStartRequest {
  label: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasskeyRegisterFinishRequest {
  request_id: String,
  credential: RegisterPublicKeyCredential,
}

#[derive(Deserialize)]
pub(crate) struct PasskeyAuthStartRequest {
  email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasskeyAuthFinishRequest {
  request_id: String,
  credential: PublicKeyCredential,
}

pub(crate) async fn passkey_register_start(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(request): Json<PasskeyRegisterStartRequest>,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return json_error(StatusCode::UNAUTHORIZED, "Login required");
  };
  if !csrf_header_ok(&user, &headers) {
    return json_error(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }

  let label = request
    .label
    .as_deref()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .unwrap_or("Passkey")
    .to_string();
  let existing =
    load_active_passkeys(&state, &user.tenant_id, &user.user_id).await;
  let exclude_credentials = existing
    .iter()
    .map(|(_, passkey)| passkey.cred_id().clone())
    .collect::<Vec<CredentialID>>();
  let display_name = if user.display_name.trim().is_empty() {
    &user.email
  } else {
    &user.display_name
  };

  let started = state.webauthn.start_passkey_registration(
    passkey_user_uuid(&user.tenant_id, &user.user_id),
    &user.email,
    display_name,
    Some(exclude_credentials),
  );
  let Ok((options, registration_state)) = started else {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not start passkey registration",
    );
  };

  let request_id = random_secret(18);
  state.passkey_registrations.lock().await.insert(
    request_id.clone(),
    PendingPasskeyRegistration {
      tenant_id: user.tenant_id,
      user_id: user.user_id,
      label,
      state: registration_state,
    },
  );

  Json(json!({ "requestId": request_id, "options": options })).into_response()
}

pub(crate) async fn passkey_register_finish(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(request): Json<PasskeyRegisterFinishRequest>,
) -> Response {
  let Some(user) = require_user(&state, &headers).await else {
    return json_error(StatusCode::UNAUTHORIZED, "Login required");
  };
  if !csrf_header_ok(&user, &headers) {
    return json_error(StatusCode::FORBIDDEN, "Invalid CSRF token");
  }

  let Some(pending) =
    state.passkey_registrations.lock().await.remove(&request.request_id)
  else {
    return json_error(StatusCode::BAD_REQUEST, "Passkey request expired");
  };
  if pending.tenant_id != user.tenant_id || pending.user_id != user.user_id {
    return json_error(StatusCode::FORBIDDEN, "Invalid passkey request");
  }

  let Ok(passkey) = state
    .webauthn
    .finish_passkey_registration(&request.credential, &pending.state)
  else {
    return json_error(StatusCode::BAD_REQUEST, "Passkey registration failed");
  };
  let credential_id = credential_id_text(&passkey);
  let Ok(exists) = repo::passkeys::credential_exists(
    &state.db.conn,
    &user.tenant_id,
    &credential_id,
  )
  .await
  else {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not save passkey",
    );
  };
  if exists {
    return json_error(StatusCode::CONFLICT, "Passkey is already registered");
  }
  let Ok(passkey_json) = serde_json::to_string(&passkey) else {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not save passkey",
    );
  };

  if repo::passkeys::insert(
    &state.db.conn,
    &user.tenant_id,
    &user.user_id,
    &credential_id,
    &passkey_json,
    &pending.label,
  )
  .await
  .is_err()
  {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not save passkey",
    );
  }

  Json(json!({ "ok": true })).into_response()
}

pub(crate) async fn passkey_auth_start(
  State(state): State<AppState>,
  Json(request): Json<PasskeyAuthStartRequest>,
) -> Response {
  let email = request.email.trim().to_lowercase();
  let Ok(tenant_id) = default_tenant_id(&state).await else {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Tenant not available",
    );
  };
  let Ok(Some(user)) =
    repo::users::find_by_email(&state.db.conn, &tenant_id, &email).await
  else {
    return json_error(StatusCode::UNAUTHORIZED, "Passkey sign-in unavailable");
  };
  if user.disabled != 0 {
    return json_error(StatusCode::UNAUTHORIZED, "Passkey sign-in unavailable");
  }

  let active = load_active_passkeys(&state, &tenant_id, &user.id).await;
  let passkeys =
    active.iter().map(|(_, passkey)| passkey.clone()).collect::<Vec<_>>();
  if passkeys.is_empty() {
    return json_error(StatusCode::UNAUTHORIZED, "Passkey sign-in unavailable");
  }
  let Ok((options, authentication_state)) =
    state.webauthn.start_passkey_authentication(&passkeys)
  else {
    return json_error(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not start passkey sign-in",
    );
  };

  let request_id = random_secret(18);
  state.passkey_authentications.lock().await.insert(
    request_id.clone(),
    PendingPasskeyAuthentication {
      tenant_id,
      user_id: user.id,
      state: authentication_state,
    },
  );

  Json(json!({ "requestId": request_id, "options": options })).into_response()
}

pub(crate) async fn passkey_auth_finish(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(request): Json<PasskeyAuthFinishRequest>,
) -> Response {
  let Some(pending) =
    state.passkey_authentications.lock().await.remove(&request.request_id)
  else {
    return json_error(StatusCode::BAD_REQUEST, "Passkey request expired");
  };

  let Ok(auth_result) = state
    .webauthn
    .finish_passkey_authentication(&request.credential, &pending.state)
  else {
    return json_error(StatusCode::UNAUTHORIZED, "Passkey sign-in failed");
  };

  let active =
    load_active_passkeys(&state, &pending.tenant_id, &pending.user_id).await;
  for (row, mut passkey) in active {
    if passkey.update_credential(&auth_result).is_some() {
      let Ok(passkey_json) = serde_json::to_string(&passkey) else {
        return json_error(
          StatusCode::INTERNAL_SERVER_ERROR,
          "Could not update passkey",
        );
      };
      let _ = repo::passkeys::update_after_authentication(
        &state.db.conn,
        &pending.tenant_id,
        &pending.user_id,
        &row.id,
        &passkey_json,
      )
      .await;
      return create_session_json_response(
        &state,
        &headers,
        &pending.tenant_id,
        &pending.user_id,
      )
      .await;
    }
  }

  json_error(StatusCode::UNAUTHORIZED, "Passkey sign-in failed")
}
