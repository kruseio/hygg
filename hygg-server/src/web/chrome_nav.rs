//! Sidenav + notification-bell building blocks for the app chrome.

use super::*;

pub(crate) fn nav_item(href: &str, label: &str, icon_name: &str) -> String {
  format!(
    r#"<a href="{}">{}<span>{}</span></a>"#,
    esc(href),
    icon(icon_name),
    esc(label)
  )
}

/// A collapsible sidenav group. `extra` carries pre-rendered links appended
/// after the built-in items (the web extension's injected pages); `""` = none.
pub(crate) fn nav_group(
  title: &str,
  icon_name: &str,
  items: &[(&str, &str, &str)],
  extra: &str,
  open: bool,
) -> String {
  let mut links = String::new();
  for (href, label, item_icon) in items {
    links.push_str(&nav_item(href, label, item_icon));
  }
  links.push_str(extra);
  format!(
    r#"<details class="nav-group"{}><summary>{}<span>{}</span>{}</summary><div class="nav-group-items">{links}</div></details>"#,
    if open { " open" } else { "" },
    icon(icon_name),
    esc(title),
    icon("chevron-down")
  )
}

/// The "Resources" sidenav group linking to the `/docs` help center. Shown to
/// everyone (signed in or not) since the docs are public install/usage guides.
pub(crate) fn docs_nav_group() -> String {
  nav_group(
    "Resources",
    "book-open",
    &[("/docs", "Documentation", "book-open")],
    "",
    true,
  )
}

/// The notification bell + dropdown for the top bar, built from the session's
/// undismissed notifications. Shows an unread badge and a dismiss control per
/// item; pure CSS `<details>` dropdown (no JS).
pub(crate) fn notif_bell(user: &WebUser) -> String {
  let count = user.notifications.len();
  let badge = if count > 0 {
    format!(r#"<span class="notif-badge">{count}</span>"#)
  } else {
    String::new()
  };
  let mut items = String::new();
  for note in &user.notifications {
    items.push_str(&format!(
      r#"<div class="notif-item notif-{sev}"><div><strong>{title}</strong><span>{body}</span></div><form method="post" action="/app/notifications/{id}/dismiss">{csrf}<button type="submit" aria-label="Dismiss">&times;</button></form></div>"#,
      sev = esc(&note.severity),
      title = esc(&note.title),
      body = esc(&note.body),
      id = esc(&note.id),
      csrf = csrf_input(user),
    ));
  }
  if items.is_empty() {
    items.push_str(r#"<div class="notif-empty">No notifications</div>"#);
  }
  format!(
    r#"<details class="notif-menu"><summary class="notif-trigger" aria-label="Notifications">{bell}{badge}</summary><div class="notif-dropdown">{items}</div></details>"#,
    bell = icon("bell"),
  )
}
