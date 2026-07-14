use super::*;

pub(crate) async fn account_passkeys_page() -> Response {
  Redirect::to("/account").into_response()
}

pub(crate) fn account_passkeys_content(
  user: &WebUser,
  passkeys: &[repo::passkeys::PasskeySummary],
) -> String {
  let label_base = if user.display_name.trim().is_empty() {
    &user.email
  } else {
    &user.display_name
  };
  // One card: the add-passkey form and the list of registered passkeys.
  format!(
    r#"<section class="panel"><h2>Passkeys</h2>
      <form class="inline-form passkey-add" id="passkey-form">
        <input id="passkey-label" name="label" placeholder="Label" value="{} passkey">
        <button id="add-passkey" data-csrf="{}" type="submit">Add passkey</button>
      </form>
      <p class="form-status" id="passkey-register-status" role="status"></p>
      {}
    </section>
    {}"#,
    esc(label_base),
    esc(&user.csrf_secret),
    passkey_table_body(passkeys, None),
    webauthn_script()
  )
}

/// The passkeys `<table>` on its own, so callers can place it inside whichever
/// panel they render (standalone for admin, merged with the add-passkey form on
/// the account page).
pub(crate) fn passkey_table_body(
  passkeys: &[repo::passkeys::PasskeySummary],
  admin: Option<(&WebUser, &str)>,
) -> String {
  let mut rows = String::new();
  for p in passkeys {
    let action = admin.map(|(user, user_id)| {
      format!(
        r#"<form method="post" action="/app/admin/users/{}/passkeys/{}/revoke">{}<button class="danger" type="submit">Revoke</button></form>"#,
        esc(user_id),
        esc(&p.id),
        csrf_input(user)
      )
    });
    rows.push_str(&format!(
      "<tr><td>{}</td><td class=\"mono\">{}</td><td>{}</td><td>{}</td></tr>",
      esc(&p.label),
      esc(&p.credential_id),
      yes_no(p.disabled),
      action.unwrap_or_default()
    ));
  }
  if rows.is_empty() {
    rows.push_str("<tr><td colspan=\"4\">No passkeys registered.</td></tr>");
  }
  format!(
    r#"<table><thead><tr><th>Label</th><th>Credential</th><th>Disabled</th><th></th></tr></thead>
      <tbody>{rows}</tbody></table>"#
  )
}

pub(crate) fn passkey_table(
  passkeys: &[repo::passkeys::PasskeySummary],
  admin: Option<(&WebUser, &str)>,
) -> String {
  format!(
    r#"<section class="panel"><h2>Passkeys</h2>
      {}</section>"#,
    passkey_table_body(passkeys, admin)
  )
}

pub(crate) fn webauthn_script() -> &'static str {
  r#"<script>
(() => {
  const b64urlToBuffer = (value) => {
    const pad = value.length % 4 === 0 ? "" : "=".repeat(4 - (value.length % 4));
    const base64 = (value + pad).replace(/-/g, "+").replace(/_/g, "/");
    const raw = atob(base64);
    const bytes = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
    return bytes.buffer;
  };

  const bufferToB64url = (buffer) => {
    const bytes = new Uint8Array(buffer);
    let raw = "";
    for (let i = 0; i < bytes.length; i += 1) raw += String.fromCharCode(bytes[i]);
    return btoa(raw).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
  };

  const preparePublicKey = (options) => {
    const publicKey = options.publicKey;
    publicKey.challenge = b64urlToBuffer(publicKey.challenge);
    if (publicKey.user && publicKey.user.id) {
      publicKey.user.id = b64urlToBuffer(publicKey.user.id);
    }
    for (const field of ["allowCredentials", "excludeCredentials"]) {
      if (publicKey[field]) {
        publicKey[field] = publicKey[field].map((credential) => ({
          ...credential,
          id: b64urlToBuffer(credential.id),
        }));
      }
    }
    return publicKey;
  };

  const credentialToJSON = (credential) => {
    const response = {};
    for (const key of [
      "clientDataJSON",
      "attestationObject",
      "authenticatorData",
      "signature",
      "userHandle",
    ]) {
      if (credential.response[key]) {
        response[key] = bufferToB64url(credential.response[key]);
      }
    }
    if (credential.response.getTransports) {
      response.transports = credential.response.getTransports();
    }
    return {
      id: credential.id,
      rawId: bufferToB64url(credential.rawId),
      type: credential.type,
      response,
      clientExtensionResults: credential.getClientExtensionResults(),
    };
  };

  const readJson = async (response) => {
    let body = {};
    try { body = await response.json(); } catch (_) {}
    if (!response.ok) throw new Error(body.error || "Request failed");
    return body;
  };

  const setStatus = (el, message, error = false) => {
    if (!el) return;
    el.textContent = message;
    el.classList.toggle("error", error);
  };

  const registerForm = document.getElementById("passkey-form");
  if (registerForm) {
    registerForm.addEventListener("submit", async (event) => {
      event.preventDefault();
      const button = document.getElementById("add-passkey");
      const status = document.getElementById("passkey-register-status");
      try {
        if (!window.PublicKeyCredential) throw new Error("Passkeys are not available in this browser");
        button.disabled = true;
        setStatus(status, "Waiting for browser prompt...");
        const label = document.getElementById("passkey-label").value;
        const start = await readJson(await fetch("/webauthn/register/start", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-csrf-token": button.dataset.csrf,
          },
          body: JSON.stringify({ label }),
        }));
        const credential = await navigator.credentials.create({
          publicKey: preparePublicKey(start.options),
        });
        await readJson(await fetch("/webauthn/register/finish", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            "x-csrf-token": button.dataset.csrf,
          },
          body: JSON.stringify({
            requestId: start.requestId,
            credential: credentialToJSON(credential),
          }),
        }));
        setStatus(status, "Passkey added.");
        window.setTimeout(() => window.location.reload(), 350);
      } catch (error) {
        setStatus(status, error.message, true);
      } finally {
        if (button) button.disabled = false;
      }
    });
  }

  const loginButton = document.getElementById("passkey-login");
  if (loginButton) {
    loginButton.addEventListener("click", async () => {
      const status = document.getElementById("passkey-login-status");
      const email = document.querySelector("input[name='email']").value;
      try {
        if (!window.PublicKeyCredential) throw new Error("Passkeys are not available in this browser");
        if (!email.trim()) throw new Error("Email is required");
        loginButton.disabled = true;
        setStatus(status, "Waiting for browser prompt...");
        const start = await readJson(await fetch("/webauthn/auth/start", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ email }),
        }));
        const request = { publicKey: preparePublicKey(start.options) };
        if (start.options.mediation) request.mediation = start.options.mediation;
        const credential = await navigator.credentials.get(request);
        const finish = await readJson(await fetch("/webauthn/auth/finish", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            requestId: start.requestId,
            credential: credentialToJSON(credential),
          }),
        }));
        window.location.assign(finish.redirectTo || "/");
      } catch (error) {
        setStatus(status, error.message, true);
      } finally {
        loginButton.disabled = false;
      }
    });
    if (loginButton.dataset.autostart === "true") {
      window.setTimeout(() => loginButton.click(), 120);
    }
  }
})();
</script>"#
}
