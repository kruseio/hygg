//! Sidenav + notification-bell building blocks for the app chrome.

use std::sync::RwLock;

use super::*;
use crate::ext::{NavAudience, NavGroup};

/// The sidenav groups the web extension contributes, as declared at install.
///
/// Process-wide rather than carried on `AppState` or the session, for the same
/// reason as the extra stylesheet: which pages a deployment advertises is a
/// property of the deployment, not of the visitor. `page` renders signed-out
/// pages with no user and no state in scope, and the groups have to look the
/// same there as they do anywhere else.
static NAV_GROUPS: RwLock<Vec<NavGroup>> = RwLock::new(Vec::new());

/// Install the extension's sidenav groups. Called when a web extension is
/// installed. Stays empty on self-host, where nothing injects any.
pub(crate) fn set_nav_groups(web_ext: &dyn crate::ext::WebExt) {
  if let Ok(mut slot) = NAV_GROUPS.write() {
    *slot = web_ext.nav_groups();
  }
}

/// The injected groups `user` may see, each rendered and paired with its sort
/// position so the chrome can merge them in among its own.
pub(crate) fn ext_nav_groups(user: Option<&WebUser>) -> Vec<(i32, String)> {
  let Ok(groups) = NAV_GROUPS.read() else {
    return Vec::new();
  };
  groups
    .iter()
    .filter(|group| !group.links.is_empty() && shows_to(group.audience, user))
    .map(|group| {
      let items: String = group
        .links
        .iter()
        .map(|link| nav_item(link.href, link.label, link.icon))
        .collect();
      (group.order, nav_group(group.title, group.icon, &[], &items, group.open))
    })
    .collect()
}

fn shows_to(audience: NavAudience, user: Option<&WebUser>) -> bool {
  match audience {
    NavAudience::Everyone => true,
    NavAudience::SignedIn => user.is_some(),
    NavAudience::Admins => user.is_some_and(|user| user.is_admin()),
  }
}

/// Order the core's groups and the extension's into one sidenav. Ties keep
/// declaration order, so an injected group sharing a core group's order sits
/// just after it.
pub(crate) fn sidenav_links(mut groups: Vec<(i32, String)>) -> String {
  groups.sort_by_key(|(order, _)| *order);
  groups.into_iter().map(|(_, html)| html).collect()
}

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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::auth::principal::Role;
  use crate::ext::{NavLink, nav_order};

  fn user(role: Role) -> WebUser {
    WebUser {
      session_id: String::new(),
      tenant_id: String::new(),
      user_id: String::new(),
      email: String::new(),
      display_name: String::new(),
      role,
      personal_sync: true,
      workspace: true,
      password_enabled: true,
      csrf_secret: String::new(),
      notifications: Vec::new(),
      nav_admin_extra: String::new(),
    }
  }

  fn group(order: i32, audience: NavAudience) -> NavGroup {
    NavGroup {
      title: "Injected",
      icon: "compass",
      links: vec![NavLink { href: "/x", label: "X", icon: "compass" }],
      order,
      open: true,
      audience,
    }
  }

  /// An injected group goes exactly where its order puts it, on either side of
  /// any core group — the placement contract an override writes against.
  #[test]
  fn order_places_a_group_among_the_core_groups() {
    let core = vec![
      (nav_order::WORKSPACE, "workspace".to_string()),
      (nav_order::RESOURCES, "resources".to_string()),
      (nav_order::ADMIN, "admin".to_string()),
    ];

    let mut lead = core.clone();
    lead.push((nav_order::WORKSPACE - 1, "injected".to_string()));
    assert_eq!(sidenav_links(lead), "injectedworkspaceresourcesadmin");

    let mut middle = core.clone();
    middle.push((nav_order::RESOURCES + 1, "injected".to_string()));
    assert_eq!(sidenav_links(middle), "workspaceresourcesinjectedadmin");

    let mut trail = core.clone();
    trail.push((nav_order::ADMIN + 1, "injected".to_string()));
    assert_eq!(sidenav_links(trail), "workspaceresourcesadmininjected");
  }

  /// A tie keeps declaration order, and the chrome appends injected groups
  /// last, so sharing a core group's order lands just after it.
  #[test]
  fn equal_order_keeps_declaration_order() {
    let groups = vec![
      (nav_order::RESOURCES, "resources".to_string()),
      (nav_order::RESOURCES, "injected".to_string()),
    ];
    assert_eq!(sidenav_links(groups), "resourcesinjected");
  }

  #[test]
  fn audience_decides_who_sees_a_group() {
    let admin = user(Role::Admin);
    let reader = user(Role::User);

    assert!(shows_to(NavAudience::Everyone, None));
    assert!(shows_to(NavAudience::Everyone, Some(&reader)));

    assert!(!shows_to(NavAudience::SignedIn, None));
    assert!(shows_to(NavAudience::SignedIn, Some(&reader)));

    assert!(!shows_to(NavAudience::Admins, None));
    assert!(!shows_to(NavAudience::Admins, Some(&reader)));
    assert!(shows_to(NavAudience::Admins, Some(&admin)));
  }

  /// The whole point of the seam: an anonymous visitor and a signed-in one see
  /// the same `Everyone` group, so it survives the walk from `/` to `/login`.
  /// Empty groups drop out rather than rendering as a bare shell.
  ///
  /// One test rather than three: `NAV_GROUPS` is process-wide, so tests that
  /// install into it have to run in sequence to stay honest.
  #[test]
  fn installed_groups_render_by_audience() {
    struct Ext;
    impl crate::ext::WebExt for Ext {
      fn nav_groups(&self) -> Vec<NavGroup> {
        vec![
          group(nav_order::WORKSPACE - 1, NavAudience::Everyone),
          group(nav_order::ADMIN, NavAudience::Admins),
          NavGroup { links: Vec::new(), ..group(0, NavAudience::Everyone) },
        ]
      }
    }
    set_nav_groups(&Ext);

    let anon = ext_nav_groups(None);
    assert_eq!(anon.len(), 1, "anonymous sees only the Everyone group");
    assert_eq!(anon[0].0, nav_order::WORKSPACE - 1);
    assert!(anon[0].1.contains("Injected"));

    assert_eq!(
      ext_nav_groups(Some(&user(Role::User))).len(),
      1,
      "a non-admin sees the same one"
    );
    assert_eq!(
      ext_nav_groups(Some(&user(Role::Admin))).len(),
      2,
      "an admin also sees the Admins group; the empty one never renders"
    );

    // Leave the global as the rest of the suite expects to find it.
    struct Bare;
    impl crate::ext::WebExt for Bare {}
    set_nav_groups(&Bare);
    assert!(ext_nav_groups(None).is_empty());
  }
}
