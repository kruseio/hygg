use sea_orm::FromQueryResult;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardMetrics {
  pub users_total: i64,
  pub users_new: i64,
  pub users_admin: i64,
  pub users_disabled: i64,
  pub devices_total: i64,
  pub devices_active: i64,
  pub devices_seen: i64,
  pub devices_revoked: i64,
  pub documents_total: i64,
  pub documents_new: i64,
  pub organization_documents: i64,
  pub storage_bytes: i64,
  pub metadata_bytes: i64,
  pub organizations_total: i64,
  pub organizations_new: i64,
  pub organization_members: i64,
  pub sync_ops: i64,
  pub active_sessions: i64,
  pub passkeys_active: i64,
  pub recovery_active: i64,
  pub role_breakdown: Vec<BreakdownRow>,
  pub client_os: Vec<BreakdownRow>,
  pub activity: Vec<ActivityRow>,
  pub resource_metrics: Vec<ResourceMetricRow>,
}

#[derive(FromQueryResult, Debug, Clone, Serialize)]
pub struct BreakdownRow {
  pub label: String,
  pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityRow {
  pub label: String,
  pub event: String,
  pub events: i64,
  pub users: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceMetricRow {
  pub label: String,
  pub total: i64,
  pub recent: i64,
  pub actors: i64,
  pub size_bytes: i64,
}
