use super::*;

pub(crate) async fn admin_dashboard_page(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Response {
  let Some(admin) = require_admin(&state, &headers).await else {
    return Redirect::to("/login").into_response();
  };
  let now = now_millis();
  let since = now.saturating_sub(DASHBOARD_RANGE_MS);
  let Ok(metrics) =
    repo::dashboard::load(&state.db.conn, &admin.tenant_id, since, now).await
  else {
    return error_page(
      StatusCode::INTERNAL_SERVER_ERROR,
      "Dashboard metrics could not be loaded",
    );
  };
  // Sampling CPU/network briefly sleeps, so collect it off the async runtime.
  let host = tokio::task::spawn_blocking(
    crate::util::host::HostMetrics::collect_blocking,
  )
  .await
  .unwrap_or_default();
  // Injected panels; none unless an override adds any.
  let extra_panels = state.web_ext.admin_dashboard_panels(&admin).await;
  page(
    "Admin dashboard",
    Some(&admin),
    admin_dashboard_content(&metrics, &host, since, now) + &extra_panels,
  )
}

pub(crate) fn admin_dashboard_content(
  metrics: &repo::dashboard::DashboardMetrics,
  host: &crate::util::host::HostMetrics,
  since: i64,
  now: i64,
) -> String {
  let range =
    format!("{} - {} UTC", format_date_utc(since), format_date_utc(now));
  let admin_detail =
    format!("{} disabled users in total", metrics.users_disabled);
  let device_detail = format!(
    "{} seen · {} revoked",
    metrics.devices_seen, metrics.devices_revoked
  );
  let docs_detail = format!(
    "{} org docs · {} new",
    metrics.organization_documents, metrics.documents_new
  );
  let org_detail = format!(
    "{} members · {} new",
    metrics.organization_members, metrics.organizations_new
  );
  format!(
    r#"<section class="dashboard-header">
      <div>
        <p class="eyebrow">Last 30 days</p>
        <h1>Dashboard</h1>
        <p class="muted">{}</p>
      </div>
      <div class="actions">
        <a class="button secondary" href="/">Back to Site</a>
        <a class="button" href="/app/admin/users">Manage users</a>
      </div>
    </section>
    <section class="metric-grid">
      {}
      {}
      {}
      {}
      {}
      {}
      {}
      {}
    </section>
    <section class="split-grid">
      {}
      {}
    </section>
    {}
    {}
    {}
    {}"#,
    esc(&range),
    metric_card(
      "Users",
      metrics.users_total,
      &format!("{} new users", metrics.users_new)
    ),
    metric_card("Admins", metrics.users_admin, &admin_detail),
    metric_card("Devices", metrics.devices_active, &device_detail),
    metric_card("Documents", metrics.documents_total, &docs_detail),
    metric_card(
      "Storage",
      format_bytes(metrics.storage_bytes + metrics.metadata_bytes),
      &format!(
        "{} documents · {} metadata",
        format_bytes(metrics.storage_bytes),
        format_bytes(metrics.metadata_bytes)
      )
    ),
    metric_card("Sync ops", metrics.sync_ops, "applied operations"),
    metric_card("Organizations", metrics.organizations_total, &org_detail),
    metric_card(
      "Security",
      metrics.active_sessions,
      &format!(
        "{} passkeys · {} active recovery",
        metrics.passkeys_active, metrics.recovery_active
      )
    ),
    breakdown_panel(
      "Access Mix",
      "All users",
      metrics.users_total,
      &metrics.role_breakdown
    ),
    breakdown_panel(
      "Client Operating Systems",
      "Web sessions",
      session_breakdown_total(&metrics.client_os),
      &metrics.client_os
    ),
    activation_funnel(metrics),
    activity_table(&metrics.activity),
    resource_table(&metrics.resource_metrics),
    host_resources_panel(host)
  )
}

pub(crate) fn activation_funnel(
  metrics: &repo::dashboard::DashboardMetrics,
) -> String {
  let rows = [
    ("Total users", metrics.users_total),
    ("Active devices", metrics.devices_active),
    ("Synced documents", metrics.documents_total),
    ("Sync operations", metrics.sync_ops),
  ];
  let mut body = String::new();
  for (label, value) in rows {
    let pct = percent(value, metrics.users_total.max(1));
    body.push_str(&format!(
      r#"<div class="funnel-row"><span>{}</span><strong>{}</strong><small>{} % of users</small></div>"#,
      esc(label),
      value,
      pct
    ));
  }
  format!(
    r#"<section class="panel"><h2>Activation Funnel</h2>
      <p class="muted">User setup and sync readiness</p>
      <div class="funnel-list">{body}</div>
    </section>"#
  )
}

pub(crate) fn activity_table(rows: &[repo::dashboard::ActivityRow]) -> String {
  let mut body = String::new();
  for row in rows {
    body.push_str(&format!(
      "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
      esc(&row.label),
      esc(&row.event),
      row.events,
      row.users
    ));
  }
  if body.is_empty() {
    body.push_str("<tr><td colspan=\"4\">No sync activity yet.</td></tr>");
  }
  format!(
    r#"<section class="panel"><h2>Top Interactions</h2>
      <table><thead><tr><th>Element</th><th>Event</th><th>Events</th><th>Users</th></tr></thead>
      <tbody>{body}</tbody></table>
    </section>"#
  )
}

pub(crate) fn resource_table(
  rows: &[repo::dashboard::ResourceMetricRow],
) -> String {
  let mut body = String::new();
  for row in rows {
    body.push_str(&format!(
      "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
      esc(&row.label),
      row.total,
      row.recent,
      row.actors,
      esc(&format_bytes(row.size_bytes))
    ));
  }
  format!(
    r#"<section class="panel"><h2>Resource Metrics</h2>
      <table><thead><tr><th>Resource</th><th>Total</th><th>Last 30 days</th><th>Users</th><th>Size</th></tr></thead>
      <tbody>{body}</tbody></table>
    </section>"#
  )
}

pub(crate) fn session_breakdown_total(
  rows: &[repo::dashboard::BreakdownRow],
) -> i64 {
  rows.iter().map(|row| row.count).sum()
}
