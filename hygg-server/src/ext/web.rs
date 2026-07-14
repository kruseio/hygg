//! An adaptable interface for overriding what the web UI shows.
//!
//! Where [`Entitlements`] answers *data* questions (may this user sync? what
//! are the limits?), [`WebExt`] lets a downstream crate or plugin inject
//! *presentation* into the core's server-rendered pages — extra nav links,
//! panels, form fields, rows and styles. Every method defaults to injecting
//! nothing, so the server renders its own plain UI on its own.
//!
//! Anything a deployment wants to say that the core would not say for itself
//! belongs here rather than in the core's own markup.
//!
//! [`Entitlements`]: super::Entitlements

use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::Denial;
use crate::repo::organizations::OrganizationRow;
use crate::web::WebUser;

/// A link an override injects into the app chrome's navigation. `icon` names
/// one of the chrome's built-in icons (e.g. `"layers"`, `"message-square"`).
#[derive(Clone, Copy, Debug)]
pub struct NavLink {
  pub href: &'static str,
  pub label: &'static str,
  pub icon: &'static str,
}

/// The web-UI injection seam consulted by the core's page handlers. See the
/// module docs. All HTML returned by these hooks is embedded verbatim — the
/// implementation escapes its own dynamic values (the core's `esc` helper is
/// public for exactly that).
#[async_trait]
pub trait WebExt: Send + Sync {
  /// Extra links appended to the sidenav's Admin group, for pages an override
  /// serves itself.
  fn admin_nav_links(&self) -> Vec<NavLink> {
    Vec::new()
  }

  /// A nav group prepended to the sidenav, for pages an override serves
  /// outside the workspace. Empty by default, and the group is omitted
  /// entirely when it stays empty.
  fn product_nav_links(&self) -> Vec<NavLink> {
    Vec::new()
  }

  /// Where a signed-in, non-admin user *without* workspace access is sent.
  /// Unreachable by default, where everyone has workspace access.
  fn no_workspace_redirect(&self) -> &'static str {
    "/account"
  }

  /// Extra CSS appended to the core stylesheet, for styling the markup these
  /// hooks inject. The core ships no rules for markup it does not render.
  fn extra_css(&self) -> &'static str {
    ""
  }

  /// Extra rows appended to the account page's summary list. Each row is
  /// embedded verbatim; match the core's `<div><span>..</span><strong>..
  /// </strong></div>` shape to line up with the rows around it.
  async fn account_rows(&self, _user: &WebUser) -> String {
    String::new()
  }

  /// Replacement header for the "Your devices" panel. `None` renders the
  /// core's plain count.
  async fn devices_panel_head(
    &self,
    _user: &WebUser,
    _active: i64,
  ) -> Option<String> {
    None
  }

  /// Attributes spliced into the "Create token" button when
  /// [`Entitlements::authorize_device_registration`] declined, so an override
  /// can explain the refusal in its own words. The core only disables the
  /// button.
  ///
  /// [`Entitlements::authorize_device_registration`]:
  ///   super::Entitlements::authorize_device_registration
  fn device_create_denied_attrs(&self, _denial: &Denial) -> String {
    String::new()
  }

  /// Extra panels prepended to the organization management pages (both the
  /// owner-facing and the admin-facing views).
  async fn org_panels(
    &self,
    _user: &WebUser,
    _org: &OrganizationRow,
  ) -> String {
    String::new()
  }

  /// Extra form fields inside the admin create-organization wizard.
  fn org_create_fields(&self) -> String {
    String::new()
  }

  /// Called after the wizard created an organization, with the submitted
  /// wizard form — the hook provisions whatever state it injected fields for.
  async fn on_org_created(
    &self,
    _user: &WebUser,
    _organization_id: &str,
    _form: &HashMap<String, String>,
  ) {
  }

  /// Extra panels appended to the admin dashboard.
  async fn admin_dashboard_panels(&self, _admin: &WebUser) -> String {
    String::new()
  }
}

/// The default web extension: injects nothing anywhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopWebExt;

#[async_trait]
impl WebExt for NoopWebExt {}
