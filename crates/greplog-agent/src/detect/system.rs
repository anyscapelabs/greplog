use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemResources {
    pub memory: MemoryInfo,
    pub cpu: CpuInfo,
    pub disk: Vec<DiskInfo>,
    pub load_avg: [f64; 3],
    pub uptime_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CpuInfo {
    pub cores: usize,
    pub global_usage_percent: f64,
    pub brand: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
}

/// Collects system resource metrics (CPU, memory, disk).
pub fn collect_system_resources(workspace: &Path) -> SystemResources {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    // Give it a tiny bit of time to measure CPU usage differences
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let disks = Disks::new_with_refreshed_list();

    // 1. Memory
    let total_bytes = sys.total_memory();
    let used_bytes = sys.used_memory();
    let mem_pct = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let memory = MemoryInfo {
        total_bytes,
        used_bytes,
        usage_percent: (mem_pct * 10.0).round() / 10.0,
    };

    // 2. CPU
    let cpus = sys.cpus();
    let cores = cpus.len();
    let global_usage = sys.global_cpu_usage() as f64;
    let brand = cpus
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cpu = CpuInfo {
        cores,
        global_usage_percent: (global_usage * 10.0).round() / 10.0,
        brand,
    };

    // 3. Disk (Find the disk containing the workspace)
    let mut workspace_disk = None;
    for disk in disks.list() {
        let mount_point = disk.mount_point();
        if workspace.starts_with(mount_point) {
            let total = disk.total_space();
            let avail = disk.available_space();
            let used = total.saturating_sub(avail);
            let pct = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            workspace_disk = Some(DiskInfo {
                mount_point: mount_point.display().to_string(),
                total_bytes: total,
                available_bytes: avail,
                usage_percent: (pct * 10.0).round() / 10.0,
            });
            break;
        }
    }

    // Fallback to first disk if not found
    let disk_info = if let Some(d) = workspace_disk {
        vec![d]
    } else if let Some(disk) = disks.list().first() {
        let total = disk.total_space();
        let avail = disk.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        vec![DiskInfo {
            mount_point: disk.mount_point().display().to_string(),
            total_bytes: total,
            available_bytes: avail,
            usage_percent: (pct * 10.0).round() / 10.0,
        }]
    } else {
        vec![]
    };

    // 4. Load average
    let load = System::load_average();
    let load_avg = [load.one, load.five, load.fifteen];

    // 5. Uptime
    let uptime_secs = System::uptime();

    SystemResources {
        memory,
        cpu,
        disk: disk_info,
        load_avg,
        uptime_secs,
    }
}
