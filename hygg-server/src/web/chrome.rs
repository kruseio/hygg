use super::*;

/// One of the chrome's built-in icons by name, as inline SVG. Public so an
/// extension's injected markup can use the same icon set as the pages around
/// it.
pub fn icon(name: &str) -> &'static str {
  match name {
    "arrow-left" => {
      r#"<svg class="icon lucide lucide-arrow-left" viewBox="0 0 24 24" aria-hidden="true"><path d="m12 19-7-7 7-7"></path><path d="M19 12H5"></path></svg>"#
    }
    "bell" => {
      r#"<svg class="icon lucide lucide-bell" viewBox="0 0 24 24" aria-hidden="true"><path d="M10.268 21a2 2 0 0 0 3.464 0"></path><path d="M3.262 15.326A1 1 0 0 0 4 17h16a1 1 0 0 0 .74-1.673C19.41 13.956 18 12.499 18 8A6 6 0 0 0 6 8c0 4.499-1.411 5.956-2.738 7.326"></path></svg>"#
    }
    "book-open" => {
      r#"<svg class="icon lucide lucide-book-open" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 7v14"></path><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"></path></svg>"#
    }
    "building" => {
      r#"<svg class="icon lucide lucide-building" viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="2" width="16" height="20" rx="2"></rect><path d="M9 22v-4h6v4"></path><path d="M8 6h.01"></path><path d="M16 6h.01"></path><path d="M8 10h.01"></path><path d="M16 10h.01"></path><path d="M8 14h.01"></path><path d="M16 14h.01"></path></svg>"#
    }
    "chevron-down" => {
      r#"<svg class="icon lucide lucide-chevron-down nav-chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"></path></svg>"#
    }
    "compass" => {
      r#"<svg class="icon lucide lucide-compass" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10"></circle><path d="m16.24 7.76-2.12 6.36-6.36 2.12 2.12-6.36z"></path></svg>"#
    }
    "copy" => {
      r#"<svg class="icon lucide lucide-copy" viewBox="0 0 24 24" aria-hidden="true"><rect x="8" y="8" width="14" height="14" rx="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg>"#
    }
    "circle-user" => {
      r#"<svg class="icon lucide lucide-circle-user-round account-avatar-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M18 20a6 6 0 0 0-12 0"></path><circle cx="12" cy="10" r="4"></circle><circle cx="12" cy="12" r="10"></circle></svg>"#
    }
    "credit-card" => {
      r#"<svg class="icon lucide lucide-credit-card" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="5" width="20" height="14" rx="2"></rect><path d="M2 10h20"></path></svg>"#
    }
    "home" => {
      r#"<svg class="icon lucide lucide-home" viewBox="0 0 24 24" aria-hidden="true"><path d="m3 11 9-8 9 8"></path><path d="M5 10v10h14V10"></path><path d="M9 20v-6h6v6"></path></svg>"#
    }
    "key-round" => {
      r#"<svg class="icon lucide lucide-key-round" viewBox="0 0 24 24" aria-hidden="true"><path d="M2.6 17.4A2 2 0 0 0 4 20h2v-2h2v-2h2l1.6-1.6"></path><circle cx="16.5" cy="7.5" r="5.5"></circle></svg>"#
    }
    "layers" => {
      r#"<svg class="icon lucide lucide-layers" viewBox="0 0 24 24" aria-hidden="true"><path d="m12 2 10 5-10 5L2 7z"></path><path d="m2 17 10 5 10-5"></path><path d="m2 12 10 5 10-5"></path></svg>"#
    }
    "layout-dashboard" => {
      r#"<svg class="icon lucide lucide-layout-dashboard" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="7" height="9" rx="1"></rect><rect x="14" y="3" width="7" height="5" rx="1"></rect><rect x="14" y="12" width="7" height="9" rx="1"></rect><rect x="3" y="16" width="7" height="5" rx="1"></rect></svg>"#
    }
    "log-in" => {
      r#"<svg class="icon lucide lucide-log-in" viewBox="0 0 24 24" aria-hidden="true"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"></path><path d="m10 17 5-5-5-5"></path><path d="M15 12H3"></path></svg>"#
    }
    "log-out" => {
      r#"<svg class="icon lucide lucide-log-out" viewBox="0 0 24 24" aria-hidden="true"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path><path d="m16 17 5-5-5-5"></path><path d="M21 12H9"></path></svg>"#
    }
    "lock-keyhole" => {
      r#"<svg class="icon lucide lucide-lock-keyhole" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="16" r="1"></circle><rect x="3" y="10" width="18" height="12" rx="2"></rect><path d="M7 10V7a5 5 0 0 1 10 0v3"></path></svg>"#
    }
    "mail" => {
      r#"<svg class="icon lucide lucide-mail" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="4" width="20" height="16" rx="2"></rect><path d="m22 7-8.97 5.7a2 2 0 0 1-2.06 0L2 7"></path></svg>"#
    }
    "menu" => {
      r#"<svg class="icon lucide lucide-menu" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16"></path><path d="M4 12h16"></path><path d="M4 18h16"></path></svg>"#
    }
    "message-square" => {
      r#"<svg class="icon lucide lucide-message-square" viewBox="0 0 24 24" aria-hidden="true"><path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4z"></path></svg>"#
    }
    "monitor" => {
      r#"<svg class="icon lucide lucide-monitor" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="3" width="20" height="14" rx="2"></rect><path d="M8 21h8"></path><path d="M12 17v4"></path></svg>"#
    }
    "settings" => {
      r#"<svg class="icon lucide lucide-settings" viewBox="0 0 24 24" aria-hidden="true"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.51a2 2 0 0 1 1-1.72l.15-.1a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>"#
    }
    "save" => {
      r#"<svg class="icon lucide lucide-save" viewBox="0 0 24 24" aria-hidden="true"><path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"></path><path d="M17 21v-7H7v7"></path><path d="M7 3v5h8"></path></svg>"#
    }
    "server" => {
      r#"<svg class="icon lucide lucide-server" viewBox="0 0 24 24" aria-hidden="true"><rect x="2" y="2" width="20" height="8" rx="2"></rect><rect x="2" y="14" width="20" height="8" rx="2"></rect><path d="M6 6h.01"></path><path d="M6 18h.01"></path></svg>"#
    }
    "share" => {
      r#"<svg class="icon lucide lucide-share-2" viewBox="0 0 24 24" aria-hidden="true"><circle cx="18" cy="5" r="3"></circle><circle cx="6" cy="12" r="3"></circle><circle cx="18" cy="19" r="3"></circle><path d="m8.59 13.51 6.83 3.98"></path><path d="m15.41 6.51-6.82 3.98"></path></svg>"#
    }
    "shield" => {
      r#"<svg class="icon lucide lucide-shield" viewBox="0 0 24 24" aria-hidden="true"><path d="M20 13c0 5-3.5 7.5-8 9-4.5-1.5-8-4-8-9V5l8-3 8 3z"></path></svg>"#
    }
    "smartphone" => {
      r#"<svg class="icon lucide lucide-smartphone" viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="2" width="12" height="20" rx="2"></rect><path d="M11 18h2"></path></svg>"#
    }
    "user-plus" => {
      r#"<svg class="icon lucide lucide-user-plus" viewBox="0 0 24 24" aria-hidden="true"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"></path><circle cx="9" cy="7" r="4"></circle><path d="M19 8v6"></path><path d="M22 11h-6"></path></svg>"#
    }
    "users" => {
      r#"<svg class="icon lucide lucide-users" viewBox="0 0 24 24" aria-hidden="true"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"></path><circle cx="9" cy="7" r="4"></circle><path d="M22 21v-2a4 4 0 0 0-3-3.87"></path><path d="M16 3.13a4 4 0 0 1 0 7.75"></path></svg>"#
    }
    _ => {
      r#"<svg class="icon lucide" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"></circle></svg>"#
    }
  }
}

/// Render the marketing "Product" sidenav group from the web extension's
/// [`product_nav_links`], or an empty string when it injects none (the
/// default). Shared by the signed-in chrome (via [`WebUser::nav_lead`]) and any
/// extension page that shows it while logged out.
///
/// [`product_nav_links`]: crate::ext::WebExt::product_nav_links
pub fn product_nav_html(web_ext: &dyn crate::ext::WebExt) -> String {
  let links = web_ext.product_nav_links();
  if links.is_empty() {
    return String::new();
  }
  let items: String =
    links.iter().map(|l| nav_item(l.href, l.label, l.icon)).collect();
  nav_group("Product", "compass", &[], &items, true)
}

/// Render a full page. The leading marketing nav comes from the signed-in
/// user's pre-rendered [`nav_lead`] (empty on self-host); anonymous pages get
/// none. Marketing handlers that must show it while logged out call
/// [`page_with_lead`] with an explicit group.
///
/// [`nav_lead`]: WebUser::nav_lead
pub fn page(title: &str, user: Option<&WebUser>, body: String) -> Response {
  let lead = user.map(|u| u.nav_lead.clone()).unwrap_or_default();
  page_with_lead(title, user, &lead, body)
}

/// Like [`page`] but with an explicit pre-rendered leading nav group, so an
/// extension's pages can show that group even for anonymous visitors (where
/// there is no signed-in user to carry it).
pub fn page_with_lead(
  title: &str,
  user: Option<&WebUser>,
  lead_nav: &str,
  body: String,
) -> Response {
  let (sidenav, topbar) = match user {
    Some(user) => {
      let admin = if user.is_admin() {
        nav_group(
          "Admin",
          "shield",
          &[
            ("/app/admin/dashboard", "Dashboard", "layout-dashboard"),
            ("/app/admin/organizations", "Admin organizations", "building"),
            ("/app/admin/users", "Admin users", "users"),
            ("/app/admin/devices", "Admin devices", "server"),
          ],
          &user.nav_admin_extra,
          true,
        )
      } else {
        String::new()
      };
      let name = if user.display_name.trim().is_empty() {
        &user.email
      } else {
        &user.display_name
      };
      let workspace = if user.has_workspace_access() {
        let items = [
          ("/app/home", "Home", "home"),
          ("/app/shares", "Shared", "mail"),
          ("/app/devices", "Devices", "smartphone"),
          ("/app/organizations", "Organizations", "building"),
        ];
        nav_group("Workspace", "home", &items, "", true)
      } else {
        String::new()
      };
      (
        format!(
          r#"<aside class="sidenav">
          <a class="brand" href="/">hygg</a>
          <nav class="sidenav-links" aria-label="Primary">
            {}
            {}
            {}
            {}
          </nav>
        </aside>"#,
          // The leading nav group is injected by the web extension; nothing
          // renders here unless an override adds one.
          lead_nav,
          workspace,
          docs_nav_group(),
          admin
        ),
        format!(
          r#"<header class="topbar">
        <label class="sidenav-toggle-button" for="sidenav-toggle" aria-label="Toggle navigation" title="Toggle navigation">{}</label>
        <div class="topbar-spacer"></div>
        <div class="nav-user">{}<details class="account-menu">
          <summary class="account-trigger" aria-label="Open account menu">
            {}
            <span class="account-trigger-name">{}</span>{}
          </summary>
          <div class="account-dropdown" role="menu">
            <div class="account-dropdown-header"><strong>{}</strong><span>{}</span></div>
            <a href="/account" role="menuitem">{}<span>Settings</span></a>
            <form method="post" action="/logout">{}<button class="dropdown-submit" type="submit">{}<span>Sign out</span></button></form>
          </div>
        </details></div></header>"#,
          icon("menu"),
          notif_bell(user),
          icon("circle-user"),
          esc(name),
          icon("chevron-down"),
          esc(name),
          esc(&user.email),
          icon("settings"),
          csrf_input(user),
          icon("log-out")
        ),
      )
    }
    None => (
      format!(
        r#"<aside class="sidenav">
        <a class="brand" href="/">hygg</a>
        <nav class="sidenav-links" aria-label="Primary">
          {}
          {}
          {}
        </nav>
      </aside>"#,
        // The leading nav group is only non-empty when the page passed
        // one; the core's own auth pages render nothing here.
        lead_nav,
        docs_nav_group(),
        nav_group(
          "Account",
          "users",
          &[
            ("/login", "Log in", "log-in"),
            ("/signup", "Sign up", "user-plus")
          ],
          "",
          true,
        )
      ),
      format!(
        r#"<header class="topbar"><label class="sidenav-toggle-button" for="sidenav-toggle" aria-label="Toggle navigation" title="Toggle navigation">{}</label><div class="topbar-spacer"></div></header>"#,
        icon("menu")
      ),
    ),
  };
  let html = Html(format!(
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>{base}{components}{docs}{responsive}{extra}</style>
</head>
<body><input class="sidenav-toggle" id="sidenav-toggle" type="checkbox" aria-label="Toggle navigation"><label class="sidenav-backdrop" for="sidenav-toggle" aria-hidden="true"></label><div class="app-shell">{sidenav}<div class="content-shell">{topbar}<main>{body}</main></div></div></body></html>"#,
    esc(title),
    base = APP_CSS_BASE,
    components = APP_CSS_COMPONENTS,
    docs = APP_CSS_DOCS,
    responsive = APP_CSS_RESPONSIVE,
    extra = extra_css(),
  ));
  if let Some(user) = user {
    ([(header::SET_COOKIE, session_cookie(&user.session_id))], html)
      .into_response()
  } else {
    html.into_response()
  }
}

/// A back-navigation link (left arrow + label) for pages reached via a POST,
/// where the browser's own back button would re-submit the form.
pub(crate) fn back_link(href: &str, label: &str) -> String {
  format!(
    r#"<a class="back-link" href="{}">{}<span>{}</span></a>"#,
    esc(href),
    icon("arrow-left"),
    esc(label)
  )
}
