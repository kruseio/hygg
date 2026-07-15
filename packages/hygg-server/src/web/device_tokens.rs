use super::*;

pub(crate) async fn create_device_for_user(
  state: &AppState,
  actor: &WebUser,
  target_user_id: &str,
  form: &HashMap<String, String>,
  server_url: &str,
  back_url: &str,
) -> Response {
  // A name is required so tokens are identifiable in the devices list; the
  // form also marks the field `required`, this is the server-side backstop.
  if field(form, "name").is_empty() {
    return error_page(StatusCode::BAD_REQUEST, "Device name is required");
  }
  let Some(target) =
    repo::users::find_by_id(&state.db.conn, &actor.tenant_id, target_user_id)
      .await
      .ok()
      .flatten()
  else {
    return error_page(StatusCode::NOT_FOUND, "User not found");
  };
  // Same decision the API registration endpoint enforces, asked about the
  // token's target user (admins can mint for other users). A hook that
  // declines supplies the wording.
  if let Err(err) = state
    .entitlements
    .authorize_device_registration(crate::ext::EntCtx {
      tenant_id: &actor.tenant_id,
      user_id: &target.id,
      is_admin: Role::parse(&target.role).is_admin(),
    })
    .await
  {
    return denial_page(err);
  }
  let Ok(device_id) = repo::devices::insert(
    &state.db.conn,
    &actor.tenant_id,
    target_user_id,
    &field(form, "name"),
    &field(form, "platform"),
  )
  .await
  else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Could not create device",
    );
  };
  let _ = repo::devices::set_default_access(
    &state.db.conn,
    &actor.tenant_id,
    &device_id,
    normalized_access(&field(form, "default_access")),
  )
  .await;
  let token = generate_token();
  let _ = repo::tokens::insert(
    &state.db.conn,
    &actor.tenant_id,
    &device_id,
    &token.prefix,
    &token.hash,
  )
  .await;
  check_user_orgs(state, &actor.tenant_id, target_user_id).await;
  device_token_page(actor, &target.email, &token.full, server_url, back_url)
}

pub(crate) async fn device_permissions_response(
  state: &AppState,
  user: &WebUser,
  device: &repo::devices::DeviceRow,
  action: &str,
  title: &str,
) -> Response {
  let books = repo::books::list_for_user(
    &state.db.conn,
    &user.tenant_id,
    &device.user_id,
  )
  .await
  .unwrap_or_default();
  let overrides =
    repo::scopes::list_for_device(&state.db.conn, &user.tenant_id, &device.id)
      .await
      .unwrap_or_default()
      .into_iter()
      .map(|row| (row.book_id, row.access))
      .collect::<HashMap<_, _>>();
  page(
    title,
    Some(user),
    device_permissions_content(user, device, &books, &overrides, action),
  )
}

pub(crate) async fn save_device_permissions(
  state: &AppState,
  tenant_id: &str,
  device_id: &str,
  form: &HashMap<String, String>,
) {
  let default_access = normalized_access(&field(form, "default_access"));
  let _ = repo::devices::set_default_access(
    &state.db.conn,
    tenant_id,
    device_id,
    default_access,
  )
  .await;
  let overrides = access_overrides_from_form(form);
  let _ = repo::scopes::replace_for_device(
    &state.db.conn,
    tenant_id,
    device_id,
    &overrides,
  )
  .await;
}

pub(crate) fn device_token_page(
  user: &WebUser,
  username: &str,
  token: &str,
  server_url: &str,
  back_url: &str,
) -> Response {
  let quick_start =
    cli_quick_start_panel(server_url, Some(username), Some(token));
  one_time_secret_page_with_note_and_extra(
    user,
    "Device API token",
    token,
    "This token is shown once. Copy it now, then connect the hygg CLI.",
    &quick_start,
    &back_link(back_url, "Back to devices"),
  )
}

/// The "Connect the CLI" panel. When `username`/`token` are known (the page
/// shown right after minting a token) the auth command is prefilled into a
/// copy-ready block; otherwise (the devices list) it shows placeholders.
/// Either way there is no manual `:sync` step — auto-sync starts after auth.
pub(crate) fn cli_quick_start_panel(
  server_url: &str,
  username: Option<&str>,
  token: Option<&str>,
) -> String {
  let user_disp = username.map(esc).unwrap_or_else(|| esc("<your-username>"));
  let connect_step = match token {
    // The token page has everything: render one ready-to-copy command with a
    // copy button (that page includes the copy script).
    Some(token) => format!(
      r#"<div class="secret-row">
        <pre class="command secret-command">:connect {server}
:auth {user} {tok}</pre>
        <button class="secondary copy-secret-button" type="button" data-copy-secret>{copy}<span>Copy</span></button>
      </div>"#,
      server = esc(server_url),
      user = user_disp,
      tok = esc(token),
      copy = icon("copy"),
    ),
    // The devices list has no token yet: show the command shape with a
    // placeholder for the token the user will paste.
    None => format!(
      r#"<pre class="quickstart-code"><code>:connect {server}
:auth {user} {placeholder}</code></pre>"#,
      server = esc(server_url),
      user = user_disp,
      placeholder = esc("<paste-copied-device-token>"),
    ),
  };
  format!(
    r#"<section class="panel cli-quickstart">
      <div class="section-title">
        <div><h2>Connect the CLI</h2>
        <p class="muted">Create one device token per client. Authenticate with your username and the token; auto-sync starts after authentication. The token then locks to that one machine.</p></div>
      </div>
      <ol class="quickstart-list">
        <li><span>1</span><div><strong>Create a device token</strong><p>Use a clear name like <code>MacBook</code> or <code>Kindle VM</code>. Copy the token when it is shown.</p></div></li>
        <li><span>2</span><div><strong>Open hygg and connect</strong>{connect_step}</div></li>
        <li><span>3</span><div><strong>Continue anywhere</strong><p>Use <code>:server-progress</code> to jump to the latest server position from another device.</p></div></li>
      </ol>
      <p class="muted">Settings are saved in <code>~/.config/hygg/.env</code>. Reading still works offline; changes queue locally and sync later.</p>
    </section>"#
  )
}

pub(crate) fn one_time_secret_page_with_note(
  user: &WebUser,
  title: &str,
  secret: &str,
  note: &str,
) -> Response {
  one_time_secret_page_with_note_and_extra(user, title, secret, note, "", "")
}

pub(crate) fn one_time_secret_page_with_note_and_extra(
  user: &WebUser,
  title: &str,
  secret: &str,
  note: &str,
  extra_html: &str,
  lead_html: &str,
) -> Response {
  page(
    title,
    Some(user),
    format!(
      r#"{}<section class="panel"><h2>{}</h2>
        <p class="muted">{}</p>
        <div class="secret-row">
          <pre class="secret">{}</pre>
          <button class="secondary copy-secret-button" type="button" data-copy-secret>{}<span>Copy</span></button>
        </div>
      </section>{}
      {}"#,
      lead_html,
      esc(title),
      esc(note),
      esc(secret),
      icon("copy"),
      extra_html,
      copy_secret_script()
    ),
  )
}

pub(crate) fn copy_secret_script() -> &'static str {
  r#"<script>
(() => {
  const fallbackCopy = (value) => {
    const input = document.createElement("textarea");
    input.value = value;
    input.setAttribute("readonly", "");
    input.style.position = "fixed";
    input.style.left = "-9999px";
    document.body.appendChild(input);
    input.select();
    document.execCommand("copy");
    input.remove();
  };

  for (const button of document.querySelectorAll("[data-copy-secret]")) {
    button.addEventListener("click", async () => {
      const secret = button.closest(".secret-row")?.querySelector("pre")?.textContent || "";
      const label = button.querySelector("span");
      const original = label?.textContent || "Copy";
      try {
        if (navigator.clipboard?.writeText) {
          await navigator.clipboard.writeText(secret);
        } else {
          fallbackCopy(secret);
        }
        if (label) label.textContent = "Copied";
      } catch (_) {
        if (label) label.textContent = "Select";
      } finally {
        window.setTimeout(() => {
          if (label) label.textContent = original;
        }, 1400);
      }
    });
  }
})();
</script>"#
}
