use serde::Serialize;

use crate::entity::organizations;

mod crud;
mod members;
mod settings;

pub use crud::*;
pub use members::*;
pub use settings::*;

#[derive(Serialize, Clone, Debug)]
pub struct OrganizationRow {
  pub id: String,
  pub tenant_id: String,
  pub name: String,
  pub slug: String,
  pub default_access: String,
  pub created_by_user_id: Option<String>,
  pub created_at: i64,
  pub updated_at: i64,
}

impl From<organizations::Model> for OrganizationRow {
  fn from(model: organizations::Model) -> Self {
    Self {
      id: model.id,
      tenant_id: model.tenant_id,
      name: model.name,
      slug: model.slug,
      default_access: model.default_access,
      created_by_user_id: model.created_by_user_id,
      created_at: model.created_at,
      updated_at: model.updated_at,
    }
  }
}

#[derive(sea_orm::FromQueryResult, Serialize, Clone, Debug)]
pub struct OrganizationMembership {
  pub id: String,
  pub name: String,
  pub slug: String,
  pub role: String,
}

#[derive(sea_orm::FromQueryResult, Serialize, Clone, Debug)]
pub struct OrganizationMember {
  pub user_id: String,
  pub email: String,
  pub display_name: String,
  pub role: String,
  pub created_at: i64,
}

/// Tenant-wide listing row for the admin organizations page (with a member
/// count subquery so the table needs only one round trip).
#[derive(sea_orm::FromQueryResult, Serialize, Clone, Debug)]
pub struct OrganizationListItem {
  pub id: String,
  pub name: String,
  pub slug: String,
  pub default_access: String,
  pub member_count: i64,
}

/// Org roles collapse to two: `owner` (full management) and `member`. Legacy
/// `admin` rows are treated as owners (they had management rights).
pub(crate) fn normalized_member_role(value: &str) -> &'static str {
  match value {
    "owner" | "admin" => "owner",
    _ => "member",
  }
}

pub(crate) fn normalized_slug(value: &str) -> String {
  let slug = value
    .trim()
    .to_lowercase()
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
    .collect::<String>()
    .split('-')
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("-");
  if slug.is_empty() { crate::util::new_id() } else { slug }
}
