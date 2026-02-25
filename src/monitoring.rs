use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::models::{
    AppState, CpuInfo, DiskInfo, JobPriority, JobStatus, JobType, MemoryInfo, MonitoringData,
    MonitoringJob, NetworkInfo, PingTest, PortInfo, Server, SystemInfo,
};
use crate::ssh::SshConnectionManager;

pub struct MonitoringService;

impl MonitoringService {
    pub async fn collect_data(
        ssh_manager: &SshConnectionManager,
        server: &Server,
    ) -> Result<MonitoringData> {
        // For local machine, collect data directly without SSH
        if server.id == "local" {
            return Self::collect_local_data().await;
        }
        let timestamp = Utc::now();
        let server_id = server.id.clone();
        let mut error_messages = vec![];

        // OPTIMIZED: Single mega-command that collects CPU, memory, load, cores, model,
        // disk, network, system info, and ports in ONE SSH call
        let mega_cmd = concat!(
            "cat /proc/stat | head -1; echo '---SEP---'; ",
            "cat /proc/loadavg; echo '---SEP---'; ",
            "nproc; echo '---SEP---'; ",
            "cat /proc/cpuinfo | grep 'model name' | head -1 | cut -d: -f2; echo '---SEP---'; ",
            "cat /proc/meminfo; echo '---SEP---'; ",
            "df -h; echo '---SEP---'; ",
            "cat /proc/net/dev; echo '---SEP---'; ",
            "hostname; echo '---SEP---'; ",
            "uname -s; echo '---SEP---'; ",
            "uname -r; echo '---SEP---'; ",
            "cat /proc/uptime; echo '---SEP---'; ",
            "uname -m; echo '---SEP---'; ",
            "(ss -tuln 2>/dev/null || netstat -tuln 2>/dev/null || echo 'no_port_info')"
        );

        let (cpu, memory, disks, network, system_info, ports) =
            match ssh_manager.execute_command(server, mega_cmd).await {
                Ok(output) => {
                    let sections: Vec<&str> = output.split("---SEP---").collect();

                    // Parse CPU (sections 0-3)
                    let cpu =
                        Self::parse_combined_cpu(&sections[..std::cmp::min(4, sections.len())]);

                    // Parse Memory (section 4)
                    let mem = if sections.len() > 4 {
                        Self::parse_meminfo(sections[4]).unwrap_or(MemoryInfo {
                            total: 0,
                            used: 0,
                            free: 0,
                            available: 0,
                            swap_total: 0,
                            swap_used: 0,
                            swap_free: 0,
                        })
                    } else {
                        MemoryInfo {
                            total: 0,
                            used: 0,
                            free: 0,
                            available: 0,
                            swap_total: 0,
                            swap_used: 0,
                            swap_free: 0,
                        }
                    };

                    // Parse Disks (section 5)
                    let disks = if sections.len() > 5 {
                        Self::parse_df_output(sections[5]).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    // Parse Network (section 6)
                    let network = if sections.len() > 6 {
                        Self::parse_net_dev(sections[6]).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    // Parse System Info (sections 7-11)
                    let system_info = SystemInfo {
                        hostname: sections.get(7).unwrap_or(&"").trim().to_string(),
                        os: sections.get(8).unwrap_or(&"").trim().to_string(),
                        kernel: sections.get(9).unwrap_or(&"").trim().to_string(),
                        uptime: sections
                            .get(10)
                            .unwrap_or(&"")
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0) as u64,
                        architecture: sections.get(11).unwrap_or(&"").trim().to_string(),
                    };

                    // Parse Ports (section 12)
                    let ports = if sections.len() > 12 {
                        let port_output = sections[12].trim();
                        if port_output != "no_port_info" {
                            Self::parse_netstat(port_output)
                                .or_else(|_| Self::parse_ss_output(port_output))
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    (cpu, mem, disks, network, system_info, ports)
                }
                Err(e) => {
                    error_messages.push(format!("Data collection: {}", e));
                    (
                        CpuInfo {
                            usage_percent: 0.0,
                            load_average: [0.0, 0.0, 0.0],
                            cores: 0,
                            model: String::new(),
                        },
                        MemoryInfo {
                            total: 0,
                            used: 0,
                            free: 0,
                            available: 0,
                            swap_total: 0,
                            swap_used: 0,
                            swap_free: 0,
                        },
                        Vec::new(),
                        Vec::new(),
                        SystemInfo {
                            hostname: String::new(),
                            os: String::new(),
                            kernel: String::new(),
                            uptime: 0,
                            architecture: String::new(),
                        },
                        Vec::new(),
                    )
                }
            };

        // Optimized: combine ping tests into a single SSH command (second and final SSH call)
        let ping_tests = match Self::run_ping_tests_combined(ssh_manager, server).await {
            Ok(p) => p,
            Err(e) => {
                error_messages.push(format!("Ping: {}", e));
                Vec::new()
            }
        };

        let data = MonitoringData {
            server_id,
            timestamp,
            cpu,
            memory,
            disks,
            network,
            ports,
            ping_tests,
            system_info,
        };

        if !error_messages.is_empty() {
            warn!(
                "⚠️ Encountered {} errors during monitoring: {}",
                error_messages.len(),
                error_messages.join(" | ")
            );
            if error_messages.len() >= 2 {
                let error = error_messages.join(" | ");
                return Err(anyhow::anyhow!(error));
            }
        }

        Ok(data)
    }

    fn parse_combined_cpu(sections: &[&str]) -> CpuInfo {
        let mut usage_percent = 0.0;
        let mut load_average = [0.0, 0.0, 0.0];
        let mut cores: u32 = 1;
        let mut model = String::new();

        if !sections.is_empty() {
            if let Ok(parsed) = Self::parse_cpu_usage(sections[0]) {
                usage_percent = parsed;
            }
        }
        if sections.len() >= 2 {
            if let Ok(load) = Self::parse_load_average(sections[1].trim()) {
                load_average = load;
            }
        }
        if sections.len() >= 3 {
            cores = sections[2].trim().parse().unwrap_or(1);
        }
        if sections.len() >= 4 {
            model = sections[3].trim().to_string();
        }

        CpuInfo {
            usage_percent,
            load_average,
            cores,
            model,
        }
    }

    async fn collect_local_data() -> Result<MonitoringData> {
        let timestamp = Utc::now();
        let server_id = "local".to_string();
        let mut error_messages = vec![];

        // Collect monitoring data for local machine
        let cpu = match Self::get_local_cpu_info().await {
            Ok(cpu) => cpu,
            Err(e) => {
                error_messages.push(format!("CPU: {}", e));
                CpuInfo {
                    usage_percent: 0.0,
                    load_average: [0.0, 0.0, 0.0],
                    cores: 0,
                    model: String::new(),
                }
            }
        };
        let memory = match Self::get_local_memory_info().await {
            Ok(mem) => mem,
            Err(e) => {
                error_messages.push(format!("Memory: {}", e));
                MemoryInfo {
                    total: 0,
                    used: 0,
                    free: 0,
                    available: 0,
                    swap_total: 0,
                    swap_used: 0,
                    swap_free: 0,
                }
            }
        };
        let disks = match Self::get_local_disk_info().await {
            Ok(d) => d,
            Err(e) => {
                error_messages.push(format!("Disks: {}", e));
                Vec::new()
            }
        };
        let network = match Self::get_local_network_info().await {
            Ok(n) => n,
            Err(e) => {
                error_messages.push(format!("Network: {}", e));
                Vec::new()
            }
        };
        let ports = match Self::get_local_port_info().await {
            Ok(p) => p,
            Err(e) => {
                error_messages.push(format!("Ports: {}", e));
                Vec::new()
            }
        };
        let system_info = match Self::get_local_system_info().await {
            Ok(s) => s,
            Err(e) => {
                error_messages.push(format!("System: {}", e));
                SystemInfo {
                    hostname: String::new(),
                    os: String::new(),
                    kernel: String::new(),
                    uptime: 0,
                    architecture: String::new(),
                }
            }
        };
        let ping_tests = match Self::run_local_ping_tests().await {
            Ok(p) => p,
            Err(e) => {
                error_messages.push(format!("Ping: {}", e));
                Vec::new()
            }
        };

        let data = MonitoringData {
            server_id,
            timestamp,
            cpu,
            memory,
            disks,
            network,
            ports,
            ping_tests,
            system_info,
        };

        if !error_messages.is_empty() {
            let error = error_messages.join(" | ");
            return Err(anyhow::anyhow!(error));
        }

        Ok(data)
    }

    pub async fn start_monitoring_loop(app_state: Arc<AppState>) -> Result<()> {
        let ssh_manager = Arc::new(SshConnectionManager::new(app_state.clone()));

        loop {
            let servers = {
                let servers = app_state.servers.read().unwrap();
                servers.clone()
            };

            for (_, server) in servers.iter() {
                // Skip paused servers
                if app_state.is_server_paused(&server.id) {
                    continue;
                }

                let now = chrono::Utc::now().timestamp() as u64;
                if server.next_monitoring <= now {
                    let server = server.clone();
                    let ssh_manager = ssh_manager.clone();
                    let app_state = app_state.clone();

                    // Create a job for this monitoring task
                    let job_id = uuid::Uuid::new_v4().to_string();
                    let job = MonitoringJob {
                        id: job_id.clone(),
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        job_type: JobType::FullCollection,
                        status: JobStatus::Pending,
                        created_at: chrono::Utc::now(),
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                        error: None,
                        metrics_collected: Vec::new(),
                        retry_count: 0,
                        priority: JobPriority::Normal,
                    };
                    app_state.add_job(job);

                    tokio::spawn(async move {
                        let start = std::time::Instant::now();
                        app_state.update_job_status(&job_id, JobStatus::Running, None, None, None);

                        match Self::monitor_server(&ssh_manager, &server, &app_state).await {
                            Ok(_) => {
                                let duration = start.elapsed().as_millis() as u64;
                                let metrics = vec![
                                    "cpu".to_string(),
                                    "memory".to_string(),
                                    "disks".to_string(),
                                    "network".to_string(),
                                    "ports".to_string(),
                                    "system".to_string(),
                                    "ping".to_string(),
                                ];
                                app_state.update_job_status(
                                    &job_id,
                                    JobStatus::Completed,
                                    None,
                                    Some(duration),
                                    Some(metrics),
                                );
                            }
                            Err(e) => {
                                let duration = start.elapsed().as_millis() as u64;
                                error!("❌ Failed to monitor server {}: {}", server.id, e);
                                app_state.update_job_status(
                                    &job_id,
                                    JobStatus::Failed,
                                    Some(e.to_string()),
                                    Some(duration),
                                    None,
                                );
                            }
                        }
                    });
                }
            }

            // Clean up inactive connections
            ssh_manager.cleanup_inactive_connections().await;

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn monitor_server(
        ssh_manager: &SshConnectionManager,
        server: &Server,
        app_state: &AppState,
    ) -> Result<()> {
        info!("🔍 Starting monitoring for server: {}", server.name);

        // Update server status to connecting
        {
            let mut servers = app_state.servers.write().unwrap();
            if let Some(s) = servers.get_mut(&server.id) {
                s.status = crate::models::ServerStatus::Connecting;
            }
        }

        match Self::collect_data(ssh_manager, server).await {
            Ok(data) => {
                info!("📊 Successfully collected data for server: {}", server.name);

                // Update server status to online
                {
                    let mut servers = app_state.servers.write().unwrap();
                    if let Some(s) = servers.get_mut(&server.id) {
                        s.status = crate::models::ServerStatus::Online;
                        s.last_seen = Some(chrono::Utc::now());
                        s.next_monitoring =
                            chrono::Utc::now().timestamp() as u64 + s.monitoring_interval.as_secs();
                    }
                }

                // Store monitoring data
                app_state.add_monitoring_data(server.id.clone(), data);
                info!("✅ Server {} monitored successfully", server.name);
            }
            Err(e) => {
                warn!(
                    "⚠️ Failed to collect data for server {}: {}",
                    server.name, e
                );

                // Update server status to error
                {
                    let mut servers = app_state.servers.write().unwrap();
                    if let Some(s) = servers.get_mut(&server.id) {
                        s.status = crate::models::ServerStatus::Error(e.to_string());
                        s.next_monitoring =
                            chrono::Utc::now().timestamp() as u64 + s.monitoring_interval.as_secs();
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_netstat(output: &str) -> Result<Vec<PortInfo>> {
        let mut ports = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for line in lines.iter().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let protocol = parts[0].to_lowercase();
                let local_address = parts[3];

                if let Some(port_str) = local_address.split(':').next_back() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        ports.push(PortInfo {
                            port,
                            protocol,
                            state: "LISTEN".to_string(),
                            process: None,
                            pid: None,
                        });
                    }
                }
            }
        }

        Ok(ports)
    }

    fn parse_ss_output(output: &str) -> Result<Vec<PortInfo>> {
        let mut ports = Vec::new();
        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let local_addr = parts[4];
                if let Some(port_str) = local_addr.split(':').next_back() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        ports.push(PortInfo {
                            port,
                            protocol: parts[0].to_lowercase(),
                            state: parts.get(1).unwrap_or(&"LISTEN").to_string(),
                            process: None,
                            pid: None,
                        });
                    }
                }
            }
        }
        Ok(ports)
    }

    async fn run_ping_tests_combined(
        ssh_manager: &SshConnectionManager,
        server: &Server,
    ) -> Result<Vec<PingTest>> {
        let targets = ["8.8.8.8", "1.1.1.1", "google.com", "github.com"];
        // Combine all ping commands into a single SSH call
        let combined = targets
            .iter()
            .map(|t| format!("ping -c 1 -W 3 {} 2>&1 || true", t))
            .collect::<Vec<_>>()
            .join("; echo '---PING_SEP---'; ");

        let mut ping_tests = Vec::new();
        match ssh_manager.execute_command(server, &combined).await {
            Ok(output) => {
                let sections: Vec<&str> = output.split("---PING_SEP---").collect();
                for (i, target) in targets.iter().enumerate() {
                    let section = sections.get(i).unwrap_or(&"");
                    if let Some(latency) = Self::extract_ping_latency(section) {
                        ping_tests.push(PingTest {
                            target: target.to_string(),
                            latency_ms: Some(latency),
                            success: true,
                            error: None,
                        });
                    } else {
                        ping_tests.push(PingTest {
                            target: target.to_string(),
                            latency_ms: None,
                            success: false,
                            error: Some("No response".to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                for target in &targets {
                    ping_tests.push(PingTest {
                        target: target.to_string(),
                        latency_ms: None,
                        success: false,
                        error: Some(e.to_string()),
                    });
                }
            }
        }
        Ok(ping_tests)
    }

    fn parse_cpu_usage(output: &str) -> Result<f64> {
        let re = Regex::new(r"cpu\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)")?;
        if let Some(caps) = re.captures(output) {
            let user: u64 = caps.get(1).unwrap().as_str().parse()?;
            let nice: u64 = caps.get(2).unwrap().as_str().parse()?;
            let system: u64 = caps.get(3).unwrap().as_str().parse()?;
            let idle: u64 = caps.get(4).unwrap().as_str().parse()?;
            let iowait: u64 = caps.get(5).unwrap().as_str().parse()?;
            let irq: u64 = caps.get(6).unwrap().as_str().parse()?;
            let softirq: u64 = caps.get(7).unwrap().as_str().parse()?;

            let total = user + nice + system + idle + iowait + irq + softirq;
            let used = total - idle;
            let usage = (used as f64 / total as f64) * 100.0;
            return Ok(usage);
        }

        let re = Regex::new(r"Cpu\(s\):\s+(\d+\.?\d*)%us")?;
        if let Some(caps) = re.captures(output) {
            return Ok(caps.get(1).unwrap().as_str().parse()?);
        }

        Err(anyhow::anyhow!("Could not parse CPU usage"))
    }

    fn parse_load_average(output: &str) -> Result<[f64; 3]> {
        let parts: Vec<&str> = output.split_whitespace().collect();
        if parts.len() >= 3 {
            Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?])
        } else {
            Ok([0.0, 0.0, 0.0])
        }
    }

    fn parse_meminfo(output: &str) -> Result<MemoryInfo> {
        let mut mem = MemoryInfo {
            total: 0,
            used: 0,
            free: 0,
            available: 0,
            swap_total: 0,
            swap_used: 0,
            swap_free: 0,
        };

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(value) = parts[1].parse::<u64>() {
                    match parts[0] {
                        "MemTotal:" => mem.total = value * 1024,
                        "MemFree:" => mem.free = value * 1024,
                        "MemAvailable:" => mem.available = value * 1024,
                        "SwapTotal:" => mem.swap_total = value * 1024,
                        "SwapFree:" => mem.swap_free = value * 1024,
                        _ => {}
                    }
                }
            }
        }

        mem.used = mem.total - mem.free;
        mem.swap_used = mem.swap_total - mem.swap_free;
        Ok(mem)
    }

    fn parse_df_output(output: &str) -> Result<Vec<DiskInfo>> {
        let mut disks = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for line in lines.iter().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let device = parts[0].to_string();
                let total = Self::parse_size(parts[1])?;
                let used = Self::parse_size(parts[2])?;
                let available = Self::parse_size(parts[3])?;
                let usage_percent = parts[4].trim_end_matches('%').parse::<f64>()?;
                let mount_point = parts[5].to_string();
                let filesystem = if parts.len() > 6 { parts[6] } else { "unknown" };

                disks.push(DiskInfo {
                    device,
                    mount_point,
                    total,
                    used,
                    free: available,
                    usage_percent,
                    filesystem: filesystem.to_string(),
                });
            }
        }
        Ok(disks)
    }

    fn parse_size(size_str: &str) -> Result<u64> {
        let size_str = size_str.to_uppercase();
        let (number, unit) = if size_str.ends_with("K") {
            (size_str.trim_end_matches('K'), 1024)
        } else if size_str.ends_with("M") {
            (size_str.trim_end_matches('M'), 1024 * 1024)
        } else if size_str.ends_with("G") {
            (size_str.trim_end_matches('G'), 1024 * 1024 * 1024)
        } else if size_str.ends_with("T") {
            (size_str.trim_end_matches('T'), 1024_u64.pow(4))
        } else {
            (size_str.as_str(), 1)
        };

        let number: f64 = number.parse()?;
        Ok((number * unit as f64) as u64)
    }

    fn parse_net_dev(output: &str) -> Result<Vec<NetworkInfo>> {
        let mut networks = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for line in lines.iter().skip(2) {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let interface = parts[0].trim().to_string();
                let stats: Vec<&str> = parts[1].split_whitespace().collect();

                if stats.len() >= 16 {
                    networks.push(NetworkInfo {
                        interface,
                        rx_bytes: stats[0].parse().unwrap_or(0),
                        tx_bytes: stats[8].parse().unwrap_or(0),
                        rx_packets: stats[1].parse().unwrap_or(0),
                        tx_packets: stats[9].parse().unwrap_or(0),
                        rx_errors: stats[2].parse().unwrap_or(0),
                        tx_errors: stats[10].parse().unwrap_or(0),
                        ip_addresses: vec![],
                    });
                }
            }
        }
        Ok(networks)
    }

    fn extract_ping_latency(output: &str) -> Option<f64> {
        let re = Regex::new(r"time=(\d+\.?\d*)").unwrap();
        if let Some(caps) = re.captures(output) {
            caps.get(1)?.as_str().parse().ok()
        } else {
            None
        }
    }

    // Local data collection functions (no SSH required)
    async fn get_local_cpu_info() -> Result<CpuInfo> {
        let output_str = tokio::fs::read_to_string("/proc/stat").await?;
        let lines: Vec<&str> = output_str.lines().collect();
        let cpu_line = lines
            .first()
            .ok_or_else(|| anyhow::anyhow!("No CPU line found"))?;

        let re = Regex::new(r"cpu\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)")?;
        if let Some(caps) = re.captures(cpu_line) {
            let user: u64 = caps.get(1).unwrap().as_str().parse()?;
            let nice: u64 = caps.get(2).unwrap().as_str().parse()?;
            let system: u64 = caps.get(3).unwrap().as_str().parse()?;
            let idle: u64 = caps.get(4).unwrap().as_str().parse()?;
            let iowait: u64 = caps.get(5).unwrap().as_str().parse()?;
            let irq: u64 = caps.get(6).unwrap().as_str().parse()?;
            let softirq: u64 = caps.get(7).unwrap().as_str().parse()?;

            let total = user + nice + system + idle + iowait + irq + softirq;
            let idle_total = idle + iowait;
            let usage_percent = if total > 0 {
                ((total - idle_total) as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let load_str = tokio::fs::read_to_string("/proc/loadavg")
                .await
                .unwrap_or_default();
            let load_parts: Vec<&str> = load_str.split_whitespace().collect();
            let load_average = [
                load_parts
                    .first()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
                load_parts
                    .get(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
                load_parts
                    .get(2)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0),
            ];

            let cores_output = tokio::process::Command::new("nproc").output().await?;
            let cores = String::from_utf8(cores_output.stdout)?
                .trim()
                .parse()
                .unwrap_or(1);

            let cpuinfo_str = tokio::fs::read_to_string("/proc/cpuinfo")
                .await
                .unwrap_or_default();
            let model = cpuinfo_str
                .lines()
                .find(|line| line.starts_with("model name"))
                .and_then(|line| line.split(':').nth(1))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            Ok(CpuInfo {
                usage_percent,
                load_average,
                cores: cores as u32,
                model,
            })
        } else {
            Err(anyhow::anyhow!("Failed to parse CPU stats"))
        }
    }

    async fn get_local_memory_info() -> Result<MemoryInfo> {
        let output_str = tokio::fs::read_to_string("/proc/meminfo").await?;
        let mut mem = MemoryInfo {
            total: 0,
            used: 0,
            free: 0,
            available: 0,
            swap_total: 0,
            swap_used: 0,
            swap_free: 0,
        };

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(value) = parts[1].parse::<u64>() {
                    match parts[0] {
                        "MemTotal:" => mem.total = value * 1024,
                        "MemFree:" => mem.free = value * 1024,
                        "MemAvailable:" => mem.available = value * 1024,
                        "SwapTotal:" => mem.swap_total = value * 1024,
                        "SwapFree:" => mem.swap_free = value * 1024,
                        _ => {}
                    }
                }
            }
        }

        mem.used = mem.total - mem.free;
        mem.swap_used = mem.swap_total - mem.swap_free;

        Ok(mem)
    }

    async fn get_local_disk_info() -> Result<Vec<DiskInfo>> {
        let output = tokio::process::Command::new("df")
            .arg("-h")
            .arg("--output=source,target,fstype,size,used,avail,pcent")
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to run df command"));
        }

        let output_str = String::from_utf8(output.stdout)?;
        let mut disks = Vec::new();

        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 7 {
                let device = parts[0].to_string();
                let mount_point = parts[1].to_string();
                let filesystem = parts[2].to_string();
                let total_str = parts[3].replace("G", "").replace("M", "").replace("K", "");
                let used_str = parts[4].replace("G", "").replace("M", "").replace("K", "");
                let free_str = parts[5].replace("G", "").replace("M", "").replace("K", "");
                let usage_str = parts[6].replace("%", "");

                if let (Ok(total), Ok(used), Ok(free), Ok(usage_percent)) = (
                    total_str.parse::<f64>(),
                    used_str.parse::<f64>(),
                    free_str.parse::<f64>(),
                    usage_str.parse::<f64>(),
                ) {
                    disks.push(DiskInfo {
                        device,
                        mount_point,
                        total: (total * 1024.0 * 1024.0 * 1024.0) as u64,
                        used: (used * 1024.0 * 1024.0 * 1024.0) as u64,
                        free: (free * 1024.0 * 1024.0 * 1024.0) as u64,
                        usage_percent,
                        filesystem,
                    });
                }
            }
        }

        Ok(disks)
    }

    async fn get_local_network_info() -> Result<Vec<NetworkInfo>> {
        let output_str = tokio::fs::read_to_string("/proc/net/dev").await?;
        let mut networks = Vec::new();

        for line in output_str.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let interface = parts[0].trim_end_matches(':').to_string();
                let rx_bytes = parts[1].parse().unwrap_or(0);
                let rx_packets = parts[2].parse().unwrap_or(0);
                let rx_errors = parts[3].parse().unwrap_or(0);
                let tx_bytes = parts[9].parse().unwrap_or(0);
                let tx_packets = parts[10].parse().unwrap_or(0);
                let tx_errors = parts[11].parse().unwrap_or(0);

                networks.push(NetworkInfo {
                    interface,
                    rx_bytes,
                    tx_bytes,
                    rx_packets,
                    tx_packets,
                    rx_errors,
                    tx_errors,
                    ip_addresses: Vec::new(), // Would need additional parsing
                });
            }
        }

        Ok(networks)
    }

    async fn get_local_port_info() -> Result<Vec<PortInfo>> {
        let output = tokio::process::Command::new("ss")
            .arg("-tuln")
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to run ss command"));
        }

        let output_str = String::from_utf8(output.stdout)?;
        let mut ports = Vec::new();

        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let state = parts[1].to_string();
                let local_addr = parts[4];
                if let Some(port_str) = local_addr.split(':').next_back() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        ports.push(PortInfo {
                            port,
                            protocol: "tcp".to_string(),
                            state,
                            process: None,
                            pid: None,
                        });
                    }
                }
            }
        }

        Ok(ports)
    }

    async fn get_local_system_info() -> Result<SystemInfo> {
        let hostname = tokio::process::Command::new("hostname")
            .output()
            .await
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();

        let os = tokio::process::Command::new("uname")
            .arg("-s")
            .output()
            .await
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();

        let kernel = tokio::process::Command::new("uname")
            .arg("-r")
            .output()
            .await
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();

        let uptime_str = tokio::fs::read_to_string("/proc/uptime")
            .await
            .unwrap_or_default();
        let uptime = uptime_str
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;

        let architecture = tokio::process::Command::new("uname")
            .arg("-m")
            .output()
            .await
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();

        Ok(SystemInfo {
            hostname,
            os,
            kernel,
            uptime,
            architecture,
        })
    }

    async fn run_local_ping_tests() -> Result<Vec<PingTest>> {
        let targets = vec!["8.8.8.8", "1.1.1.1", "google.com", "github.com"];

        // Run all pings concurrently with timeout
        let mut handles = Vec::new();
        for target in &targets {
            let t = target.to_string();
            handles.push(tokio::spawn(
                async move { Self::ping_local_target(&t).await },
            ));
        }

        let mut ping_tests = Vec::new();
        for (i, handle) in handles.into_iter().enumerate() {
            match handle.await {
                Ok(result) => ping_tests.push(result),
                Err(e) => ping_tests.push(PingTest {
                    target: targets[i].to_string(),
                    latency_ms: None,
                    success: false,
                    error: Some(format!("Task failed: {}", e)),
                }),
            }
        }

        Ok(ping_tests)
    }

    async fn ping_local_target(target: &str) -> PingTest {
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(3),
            tokio::process::Command::new("ping")
                .args(["-c", "1", "-W", "2", target])
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if let Some(latency) = Self::extract_ping_latency(&output_str) {
                    PingTest {
                        target: target.to_string(),
                        latency_ms: Some(latency),
                        success: true,
                        error: None,
                    }
                } else {
                    PingTest {
                        target: target.to_string(),
                        latency_ms: None,
                        success: false,
                        error: Some("Could not parse latency".to_string()),
                    }
                }
            }
            Ok(Err(e)) => PingTest {
                target: target.to_string(),
                latency_ms: None,
                success: false,
                error: Some(e.to_string()),
            },
            Err(_) => PingTest {
                target: target.to_string(),
                latency_ms: None,
                success: false,
                error: Some("Ping timed out".to_string()),
            },
        }
    }
}
