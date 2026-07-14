//! Process and host resource metrics (CPU, memory, disk, network) for the admin
//! dashboard. A snapshot is taken per request: CPU and network both need two
//! samples a short interval apart, so collection briefly sleeps and must run on
//! a blocking thread (see `collect_blocking`).

use sysinfo::{
  Disks, MINIMUM_CPU_UPDATE_INTERVAL, Networks, ProcessesToUpdate, System,
  get_current_pid,
};

/// A point-in-time view of how much the server process and its host are using.
#[derive(Debug, Clone, Copy, Default)]
pub struct HostMetrics {
  /// This process' CPU usage, in percent (can exceed 100 across cores).
  pub cpu_percent: f32,
  /// This process' resident memory, in bytes.
  pub memory_bytes: u64,
  /// Total system memory, in bytes.
  pub memory_total_bytes: u64,
  /// Used space on the primary volume, in bytes.
  pub disk_used_bytes: u64,
  /// Total space on the primary volume, in bytes.
  pub disk_total_bytes: u64,
  /// Receive throughput across interfaces over the sample window, bytes/sec.
  pub net_rx_per_sec: u64,
  /// Transmit throughput across interfaces over the sample window, bytes/sec.
  pub net_tx_per_sec: u64,
}

/// Collect a snapshot. Blocks for `MINIMUM_CPU_UPDATE_INTERVAL` (~200ms) to
/// take the second CPU/network sample, so callers should run this off the async
/// runtime — e.g. `tokio::task::spawn_blocking(HostMetrics::collect_blocking)`.
impl HostMetrics {
  pub fn collect_blocking() -> Self {
    let pid = get_current_pid().ok();
    let mut system = System::new();
    system.refresh_memory();
    if let Some(pid) = pid {
      system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }
    // Networks needs a baseline before deltas are meaningful.
    let mut networks = Networks::new_with_refreshed_list();

    let interval = MINIMUM_CPU_UPDATE_INTERVAL;
    std::thread::sleep(interval);

    if let Some(pid) = pid {
      system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }
    networks.refresh(true);

    let (cpu_percent, memory_bytes) = pid
      .and_then(|pid| system.process(pid))
      .map(|process| (process.cpu_usage(), process.memory()))
      .unwrap_or((0.0, 0));

    let (disk_used_bytes, disk_total_bytes) = primary_disk_usage();

    let secs = interval.as_secs_f64().max(0.001);
    let rx: u64 = networks.values().map(|net| net.received()).sum();
    let tx: u64 = networks.values().map(|net| net.transmitted()).sum();

    HostMetrics {
      cpu_percent,
      memory_bytes,
      memory_total_bytes: system.total_memory(),
      disk_used_bytes,
      disk_total_bytes,
      net_rx_per_sec: (rx as f64 / secs) as u64,
      net_tx_per_sec: (tx as f64 / secs) as u64,
    }
  }
}

/// Used/total bytes for the primary volume: the disk mounted at `/` when
/// present, otherwise the disk with the most total space. Returns zeros when no
/// disks are reported.
fn primary_disk_usage() -> (u64, u64) {
  let disks = Disks::new_with_refreshed_list();
  let primary = disks
    .iter()
    .find(|disk| disk.mount_point() == std::path::Path::new("/"))
    .or_else(|| disks.iter().max_by_key(|disk| disk.total_space()));
  match primary {
    Some(disk) => {
      let total = disk.total_space();
      let used = total.saturating_sub(disk.available_space());
      (used, total)
    }
    None => (0, 0),
  }
}
