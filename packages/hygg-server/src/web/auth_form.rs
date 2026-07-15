use super::*;

#[derive(Clone, Copy, Default)]
pub(crate) struct AuthFormValues<'a> {
  pub(crate) email: &'a str,
  pub(crate) display_name: &'a str,
  pub(crate) password: &'a str,
}

pub(crate) const PASSWORD_COMPLEXITY_MESSAGE: &str =
  "Password must be at least 8 characters.";

pub(crate) fn password_complexity_error(
  password: &str,
) -> Option<&'static str> {
  if password.len() < 8 { Some(PASSWORD_COMPLEXITY_MESSAGE) } else { None }
}

// A login/register page is built from many independent, mostly-optional bits
// (title, action, button label, error, extra markup, two feature toggles, and
// the pre-filled field values); bundling them into a struct would not make the
// call sites clearer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn auth_page(
  title: &str,
  action: &str,
  button: &str,
  error: Option<&str>,
  extra: &str,
  display_name: bool,
  passkey_login: bool,
  values: AuthFormValues<'_>,
) -> Response {
  let error_html = error
    .map(|e| {
      format!(
        r#"<div class="toast-stack"><div class="toast toast-error" role="alert">{}</div></div>"#,
        esc(e)
      )
    })
    .unwrap_or_default();
  let display_name_input = if display_name {
    format!(
      r#"<input name="display_name" placeholder="Display name" value="{}">"#,
      esc(values.display_name)
    )
  } else {
    String::new()
  };
  let passkey_button = if passkey_login {
    r#"<button class="secondary" type="button" id="passkey-login">Use passkey</button>"#
  } else {
    ""
  };
  let passkey_status = if passkey_login {
    r#"<p class="form-status" id="passkey-login-status" role="status"></p>"#
  } else {
    ""
  };
  let script = if passkey_login { webauthn_script() } else { "" };
  let password_autocomplete =
    if display_name { "new-password" } else { "current-password" };
  let password_policy_attr =
    if display_name { r#" data-password-policy="signup""# } else { "" };
  let validation_script =
    if display_name { password_validation_script() } else { "" };
  page(
    title,
    None,
    format!(
      r#"<section class="panel auth"><h1>{}</h1>{error_html}{extra}
        <form method="post" action="{}" class="stack"{password_policy_attr}>
          <input name="email" type="email" autocomplete="email" placeholder="Email" value="{}">
          {display_name_input}
          <input name="password" type="password" autocomplete="{password_autocomplete}" placeholder="Password" minlength="8" value="{}">
          <div class="button-row"><button type="submit">{}</button>{passkey_button}</div>{passkey_status}
        </form>
      </section>{script}{validation_script}"#,
      esc(title),
      esc(action),
      esc(values.email),
      esc(values.password),
      esc(button)
    ),
  )
}

pub(crate) fn password_validation_script() -> &'static str {
  r#"<script>
(() => {
  const form = document.querySelector("[data-password-policy='signup']");
  if (!form) return;
  const password = form.querySelector("input[name='password']");
  const showToast = (message) => {
    let stack = document.querySelector(".toast-stack");
    if (!stack) {
      stack = document.createElement("div");
      stack.className = "toast-stack";
      document.body.appendChild(stack);
    }
    stack.innerHTML = "";
    const toast = document.createElement("div");
    toast.className = "toast toast-error";
    toast.setAttribute("role", "alert");
    toast.textContent = message;
    stack.appendChild(toast);
  };
  form.addEventListener("submit", (event) => {
    if (!password || password.value.trim().length >= 8) return;
    event.preventDefault();
    password.setAttribute("aria-invalid", "true");
    password.focus();
    showToast("Password must be at least 8 characters.");
  });
  password?.addEventListener("input", () => {
    if (password.value.trim().length >= 8) {
      password.removeAttribute("aria-invalid");
    }
  });
})();
</script>"#
}
