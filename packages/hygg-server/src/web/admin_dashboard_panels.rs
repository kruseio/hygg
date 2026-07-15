use super::*;

/// Live process/host resources: CPU, memory, disk and network for the running
/// server, shown as a metric-card row on the admin dashboard.
pub(crate) fn host_resources_panel(
  host: &crate::util::host::HostMetrics,
) -> String {
  let memory_detail = if host.memory_total_bytes > 0 {
    format!("of {} RAM", format_bytes(host.memory_total_bytes as i64))
  } else {
    "resident set".to_string()
  };
  let disk_value = format!(
    "{} / {}",
    format_bytes(host.disk_used_bytes as i64),
    format_bytes(host.disk_total_bytes as i64)
  );
  let network_value = format!(
    "↓ {}/s · ↑ {}/s",
    format_bytes(host.net_rx_per_sec as i64),
    format_bytes(host.net_tx_per_sec as i64)
  );
  format!(
    r#"<section class="panel"><h2>Server Resources</h2>
      <p class="muted">Live usage for this server process and its host.</p>
      <div class="metric-grid">
        {}
        {}
        {}
        {}
      </div>
    </section>"#,
    metric_card(
      "Process CPU",
      format!("{:.1}%", host.cpu_percent),
      "server process"
    ),
    metric_card(
      "Process memory",
      format_bytes(host.memory_bytes as i64),
      &memory_detail
    ),
    metric_card("Disk", disk_value, "primary volume"),
    metric_card("Network", network_value, "current throughput"),
  )
}

pub(crate) fn metric_card<V: std::fmt::Display>(
  label: &str,
  value: V,
  detail: &str,
) -> String {
  format!(
    r#"<div class="metric-card"><span>{}</span><strong>{}</strong><small>{}</small></div>"#,
    esc(label),
    esc(&value.to_string()),
    esc(detail)
  )
}

pub(crate) fn breakdown_panel(
  title: &str,
  subtitle: &str,
  total: i64,
  rows: &[repo::dashboard::BreakdownRow],
) -> String {
  let mut body = String::new();
  for row in rows {
    let pct = percent(row.count, total);
    body.push_str(&format!(
      r#"<div class="breakdown-row">
        <div><strong>{}</strong><span>{} %</span></div>
        <div class="bar"><span style="width:{}%"></span></div>
        <small>{}</small>
      </div>"#,
      esc(&row.label),
      pct,
      pct,
      row.count
    ));
  }
  if body.is_empty() {
    body.push_str(r#"<p class="muted">No data yet.</p>"#);
  }
  format!(
    r#"<section class="panel"><div class="section-title"><div>
      <h2>{}</h2><p class="muted">{}</p></div><strong>{}</strong></div>{body}</section>"#,
    esc(title),
    esc(subtitle),
    total
  )
}
