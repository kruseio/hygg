//! Who may see a document. Correlated predicates over `books`, shared by the
//! library listing and the storage rollups so both agree on one definition of
//! visibility rather than drifting apart.

use sea_orm::sea_query::{Expr, Query, SimpleExpr};
use sea_orm::{ColumnTrait, Condition};

use crate::entity::{books, document_shares, organization_members};

/// Membership of the organization that owns the book. A personal book has a
/// NULL `organization_id`, which never equals a member row, so this is false
/// for them without a separate guard.
fn org_member(user_id: &str) -> SimpleExpr {
  use organization_members::{Column as Om, Entity as OmEntity};
  Expr::exists(
    Query::select()
      .expr(Expr::val(1))
      .from(OmEntity)
      .and_where(
        Expr::col((OmEntity, Om::TenantId))
          .equals((books::Entity, books::Column::TenantId)),
      )
      .and_where(
        Expr::col((OmEntity, Om::OrganizationId))
          .equals((books::Entity, books::Column::OrganizationId)),
      )
      .and_where(Expr::col((OmEntity, Om::UserId)).eq(user_id))
      .to_owned(),
  )
}

/// A share of this document that the recipient has accepted. Pending and
/// revoked shares grant nothing.
fn accepted_share(user_id: &str) -> SimpleExpr {
  use document_shares::{Column as Ds, Entity as DsEntity};
  Expr::exists(
    Query::select()
      .expr(Expr::val(1))
      .from(DsEntity)
      .and_where(
        Expr::col((DsEntity, Ds::TenantId))
          .equals((books::Entity, books::Column::TenantId)),
      )
      .and_where(
        Expr::col((DsEntity, Ds::ContentHash))
          .equals((books::Entity, books::Column::ContentHash)),
      )
      .and_where(Expr::col((DsEntity, Ds::ToUserId)).eq(user_id))
      .and_where(Expr::col((DsEntity, Ds::Status)).eq("accepted"))
      .to_owned(),
  )
}

/// Books the user owns outright or reaches through their organization.
pub(super) fn owned_or_org(user_id: &str) -> Condition {
  Condition::any()
    .add(books::Column::OwnerUserId.eq(user_id))
    .add(org_member(user_id))
}

/// Every book the user may read: owned, reached via their organization, or
/// shared with them and accepted.
pub(super) fn accessible(user_id: &str) -> Condition {
  owned_or_org(user_id).add(accepted_share(user_id))
}
