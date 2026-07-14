//! An adaptable interface for overriding who may do what.
//!
//! The core consults an `Arc<dyn Entitlements>` held in [`AppState`] at every
//! point where access could be limited. Every method defaults to granting full,
//! unlimited access, so the server on its own (with [`NoopEntitlements`]) never
//! restricts anyone — the seam exists so a downstream crate or plugin can
//! override individual answers without the core knowing why.
//!
//! An override that declines a request supplies its own wording via
//! [`Denial`], which the core relays untouched. Nothing here — no policy, no
//! vocabulary, no copy — is the core's concern.
//!
//! [`AppState`]: crate::state::AppState
//! [`Denial`]: crate::error::Denial

use async_trait::async_trait;

use crate::error::{AppError, Denial};

mod web;
pub use web::{NavLink, NoopWebExt, WebExt};

/// Identity of the authenticated user a hook is being asked about. Borrowed for
/// the duration of the call; the implementation looks up whatever state it
/// needs from its own store keyed by these ids.
#[derive(Clone, Copy, Debug)]
pub struct EntCtx<'a> {
  pub tenant_id: &'a str,
  pub user_id: &'a str,
  pub is_admin: bool,
}

/// A storage-consuming upload being authorized. `organization_id` is set when
/// the document belongs to an organization's shared pool (its quota applies)
/// and `None` for a personal upload (the uploader's quota applies).
#[derive(Clone, Copy, Debug)]
pub struct UploadCtx<'a> {
  pub tenant_id: &'a str,
  pub user_id: &'a str,
  pub organization_id: Option<&'a str>,
  pub content_hash: &'a str,
  /// Size in bytes the upload would occupy in the pool.
  pub requested_size: i64,
}

/// An organization-document access being resolved. The core has already
/// confirmed membership + permission grants; the hook only decides whether a
/// seat/device cap excludes this member or device.
#[derive(Clone, Copy, Debug)]
pub struct OrgCapCtx<'a> {
  pub tenant_id: &'a str,
  pub organization_id: &'a str,
  pub user_id: &'a str,
  /// The syncing device (API only); `None` for web requests.
  pub device_id: Option<&'a str>,
}

/// The per-request entitlement decision attached to a [`Principal`].
///
/// [`Principal`]: crate::auth::Principal
#[derive(Clone, Copy, Debug)]
pub struct Decision {
  /// Whether the user may sync their own (personal, non-org) library. Org
  /// documents are covered separately by their org seat.
  pub personal_sync: bool,
  /// Whether the signed-in web user gets the workspace UI (home / devices /
  /// organizations). Granted unless an override says otherwise.
  pub workspace: bool,
}

impl Decision {
  /// The default decision: full personal access.
  pub fn full() -> Self {
    Self { personal_sync: true, workspace: true }
  }
}

/// Who a sharing question is being asked about: the caller themself, or the
/// other user in the exchange. An override needs the difference to word a
/// refusal correctly — "you can't" and "they can't" are not the same sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareSubject {
  /// The user making the request.
  Caller,
  /// The user on the other end of the share.
  OtherParty,
}

/// An organization a limit question is being asked about.
#[derive(Clone, Copy, Debug)]
pub struct OrgCtx<'a> {
  pub tenant_id: &'a str,
  pub organization_id: &'a str,
}

/// The resource budgets applied to an organization, for usage meters, quota
/// notifications and seat checks. `None` = unlimited (the default for every
/// resource).
#[derive(Clone, Copy, Debug)]
pub struct OrgLimits {
  pub seats: Option<i64>,
  pub storage_bytes: Option<i64>,
  pub devices: Option<i64>,
}

impl OrgLimits {
  /// The default result: nothing is limited.
  pub fn unlimited() -> Self {
    Self { seats: None, storage_bytes: None, devices: None }
  }
}

/// Whether an organization's seat/device caps exclude the member/device under
/// consideration.
#[derive(Clone, Copy, Debug)]
pub struct OrgCaps {
  pub member_capped: bool,
  pub device_capped: bool,
}

impl OrgCaps {
  /// The default result: no caps.
  pub fn uncapped() -> Self {
    Self { member_capped: false, device_capped: false }
  }
}

/// The interface consulted by the core. See the module docs. Every method
/// defaults to granting access, so the core is fully functional with
/// [`NoopEntitlements`] alone and an override supplies only the hooks it needs.
///
/// Methods returning `Result<(), AppError>` admit the request with `Ok(())`.
/// To decline, prefer [`AppError::Denied`] with a [`Denial`] carrying the
/// wording (and any link) the caller should see — the core has none of its own.
///
/// [`Denial`]: crate::error::Denial
#[async_trait]
pub trait Entitlements: Send + Sync {
  /// Resolve what a user may do this request (attached to the `Principal`).
  async fn resolve(&self, _cx: EntCtx<'_>) -> Decision {
    Decision::full()
  }

  /// The wording to turn a caller away from the sync API with, when [`resolve`]
  /// withheld their own-library sync *and* they hold no organization seat.
  ///
  /// Unreachable unless an override withheld something, so the default is only
  /// a backstop. An override that gates sync should answer here too, or its
  /// readers will show this instead of its own explanation.
  ///
  /// [`resolve`]: Self::resolve
  async fn sync_denial(&self, _cx: EntCtx<'_>) -> Denial {
    Denial::new("This server does not sync for this account.")
  }

  /// Decide whether a new device may be registered.
  async fn authorize_device_registration(
    &self,
    _cx: EntCtx<'_>,
  ) -> Result<(), AppError> {
    Ok(())
  }

  /// Decide whether a storage-consuming upload may proceed against the owning
  /// pool's budget.
  async fn authorize_upload(&self, _cx: UploadCtx<'_>) -> Result<(), AppError> {
    Ok(())
  }

  /// Apply an organization's seat/device caps during org-document access
  /// resolution. The permission grants themselves stay in the core.
  async fn org_caps(&self, _cx: OrgCapCtx<'_>) -> OrgCaps {
    OrgCaps::uncapped()
  }

  /// Decide whether a user may participate in peer document sharing at all —
  /// as the sender *or* the recipient of a share. Everyone may by default.
  /// `subject` says whether the question is about the caller or the other
  /// party, so a refusal can be worded for the right person.
  async fn authorize_share_participant(
    &self,
    _cx: EntCtx<'_>,
    _subject: ShareSubject,
  ) -> Result<(), AppError> {
    Ok(())
  }

  /// The maximum number of *active* document shares a user may hold in one
  /// direction — counted separately for outgoing (pending+accepted) and
  /// incoming (accepted) shares. `None` = unlimited (the default).
  async fn share_limit(&self, _cx: EntCtx<'_>) -> Option<i64> {
    None
  }

  /// A free-form account label for `GET /api/v1/me` and the account page,
  /// shown verbatim. `None` (the default) shows nothing.
  async fn account_label(&self, _cx: EntCtx<'_>) -> Option<String> {
    None
  }

  /// The user's personal storage budget in bytes for the home-page usage
  /// meter, or `None` when storage is unlimited (the default).
  async fn storage_limit(&self, _cx: EntCtx<'_>) -> Option<i64> {
    None
  }

  /// The organization's budgets for usage meters, quota notifications and the
  /// seat check. Unlimited by default.
  async fn org_limits(&self, _cx: OrgCtx<'_>) -> OrgLimits {
    OrgLimits::unlimited()
  }
}

/// The default: everything is granted, nothing is capped. Installed by
/// [`AppState::new`], so the server runs fully open when used standalone.
///
/// [`AppState::new`]: crate::state::AppState::new
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopEntitlements;

#[async_trait]
impl Entitlements for NoopEntitlements {}
