use super::*;

pub(crate) fn json_error(status: StatusCode, message: &str) -> Response {
  (status, Json(json!({ "error": message }))).into_response()
}

pub(crate) fn csrf_header_ok(user: &WebUser, headers: &HeaderMap) -> bool {
  headers.get("x-csrf-token").and_then(|value| value.to_str().ok())
    == Some(user.csrf_secret.as_str())
}

pub(crate) fn session_metadata(
  headers: &HeaderMap,
) -> (Option<String>, Option<String>) {
  let ip = headers
    .get("x-forwarded-for")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(',').next())
    .and_then(|value| bounded_metadata(value, 96))
    .or_else(|| {
      headers
        .get("x-real-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| bounded_metadata(value, 96))
    });
  let user_agent = headers
    .get(header::USER_AGENT)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| bounded_metadata(value, 512));
  (ip, user_agent)
}

pub(crate) fn bounded_metadata(
  value: &str,
  max_chars: usize,
) -> Option<String> {
  let value = value.trim();
  if value.is_empty() {
    None
  } else {
    Some(value.chars().take(max_chars).collect())
  }
}

pub(crate) fn request_base_url(headers: &HeaderMap) -> String {
  let proto = headers
    .get("x-forwarded-proto")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(',').next())
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .unwrap_or("http");
  let host = headers
    .get("x-forwarded-host")
    .or_else(|| headers.get(header::HOST))
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split(',').next())
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .unwrap_or("localhost:3032");
  format!("{proto}://{host}").trim_end_matches('/').to_string()
}

pub(crate) fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
  let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
  for part in cookie.split(';') {
    let (key, value) = part.trim().split_once('=')?;
    if key == name {
      return Some(value.to_string());
    }
  }
  None
}

pub(crate) fn session_cookie(session_id: &str) -> String {
  format!(
    "{SESSION_COOKIE}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
    session_id,
    SESSION_TTL_MS / 1000
  )
}

pub(crate) fn delete_cookie() -> String {
  format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax")
}

pub fn random_secret(bytes: usize) -> String {
  let mut buf = vec![0u8; bytes];
  rand::thread_rng().fill_bytes(&mut buf);
  URL_SAFE_NO_PAD.encode(buf)
}

pub fn csrf_input(user: &WebUser) -> String {
  format!(
    r#"<input type="hidden" name="csrf" value="{}">"#,
    esc(&user.csrf_secret)
  )
}

pub fn csrf_ok(user: &WebUser, form: &HashMap<String, String>) -> bool {
  form.get("csrf") == Some(&user.csrf_secret)
}

pub fn field(form: &HashMap<String, String>, key: &str) -> String {
  form.get(key).map(|s| s.trim().to_string()).unwrap_or_default()
}
