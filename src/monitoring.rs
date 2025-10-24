use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use std::sync::Arc;
use tracing::{info, warn, error};

use crate::models::{
    CpuInfo, DiskInfo, MemoryInfo, MonitoringData, NetworkInfo, PingTest, PortInfo, SystemInfo, Server, AppState,
};
use crate::ssh::SshConnectionManager;

pub struct MonitoringService;

impl MonitoringService {
    pub async fn collect_data(ssh_manager: &SshConnectionManager, server: &Server) -> Result<MonitoringData> {
        // For local machine, collect data directly without SSH
        if server.id == "local" {
            return Self::collect_local_data().await;
        }
        let timestamp = Utc::now();
        let server_id = server.id.clone();
        let mut error_messages = vec![];

        // Collect monitoring data sequentially
        let cpu = match Self::get_cpu_info(ssh_manager, server).await {
            Ok(cpu) => cpu,
            Err(e) => { error_messages.push(format!("CPU: {}", e)); CpuInfo { usage_percent: 0.0, load_average: [0.0, 0.0, 0.0], cores: 0, model: String::new() } }
        };
        let memory = match Self::get_memory_info(ssh_manager, server).await {
            Ok(mem) => mem,
            Err(e) => { error_messages.push(format!("Memory: {}", e)); MemoryInfo { total: 0, used: 0, free: 0, available: 0, swap_total: 0, swap_used: 0, swap_free: 0 } }
        };
        let disks = match Self::get_disk_info(ssh_manager, server).await {
            Ok(d) => d,
            Err(e) => { error_messages.push(format!("Disks: {}", e)); Vec::new() }
        };
        let network = match Self::get_network_info(ssh_manager, server).await {
            Ok(n) => n,
            Err(e) => { error_messages.push(format!("Network: {}", e)); Vec::new() }
        };
        let ports = match Self::get_port_info(ssh_manager, server).await {
            Ok(p) => p,
            Err(e) => { error_messages.push(format!("Ports: {}", e)); Vec::new() }
        };
        let system_info = match Self::get_system_info(ssh_manager, server).await {
            Ok(s) => s,
            Err(e) => { error_messages.push(format!("System: {}", e)); SystemInfo { hostname: String::new(), os: String::new(), kernel: String::new(), uptime: 0, architecture: String::new() } }
        };
        let ping_tests = match Self::run_ping_tests(ssh_manager, server).await {
            Ok(p) => p,
            Err(e) => { error_messages.push(format!("Ping: {}", e)); Vec::new() }
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
            warn!("⚠️ Encountered {} errors during monitoring: {}", error_messages.len(), error_messages.join(" | "));
            // If we have too many errors, return an error instead of partial data
            if error_messages.len() >= 3 {
                let error = error_messages.join(" | ");
                return Err(anyhow::anyhow!(error));
            }
        }

        Ok(data)
    }

    async fn collect_local_data() -> Result<MonitoringData> {
        let timestamp = Utc::now();
        let server_id = "local".to_string();
        let mut error_messages = vec![];

        // Collect monitoring data for local machine
        let cpu = match Self::get_local_cpu_info().await {
            Ok(cpu) => cpu,
            Err(e) => { error_messages.push(format!("CPU: {}", e)); CpuInfo { usage_percent: 0.0, load_average: [0.0, 0.0, 0.0], cores: 0, model: String::new() } }
        };
        let memory = match Self::get_local_memory_info().await {
            Ok(mem) => mem,
            Err(e) => { error_messages.push(format!("Memory: {}", e)); MemoryInfo { total: 0, used: 0, free: 0, available: 0, swap_total: 0, swap_used: 0, swap_free: 0 } }
        };
        let disks = match Self::get_local_disk_info().await {
            Ok(d) => d,
            Err(e) => { error_messages.push(format!("Disks: {}", e)); Vec::new() }
        };
        let network = match Self::get_local_network_info().await {
            Ok(n) => n,
            Err(e) => { error_messages.push(format!("Network: {}", e)); Vec::new() }
        };
        let ports = match Self::get_local_port_info().await {
            Ok(p) => p,
            Err(e) => { error_messages.push(format!("Ports: {}", e)); Vec::new() }
        };
        let system_info = match Self::get_local_system_info().await {
            Ok(s) => s,
            Err(e) => { error_messages.push(format!("System: {}", e)); SystemInfo { hostname: String::new(), os: String::new(), kernel: String::new(), uptime: 0, architecture: String::new() } }
        };
        let ping_tests = match Self::run_local_ping_tests().await {
            Ok(p) => p,
            Err(e) => { error_messages.push(format!("Ping: {}", e)); Vec::new() }
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
                let now = chrono::Utc::now().timestamp() as u64;
                if server.next_monitoring <= now {
                    let server = server.clone();
                    let ssh_manager = ssh_manager.clone();
                    let app_state = app_state.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::monitor_server(&ssh_manager, &server, &app_state).await {
                            error!("❌ Failed to monitor server {}: {}", server.id, e);
                        }
                    });
                }
            }
            
            // Clean up inactive connections
            ssh_manager.cleanup_inactive_connections().await;
            
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn monitor_server(ssh_manager: &SshConnectionManager, server: &Server, app_state: &AppState) -> Result<()> {
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
                        s.next_monitoring = chrono::Utc::now().timestamp() as u64 + s.monitoring_interval.as_secs();
                    }
                }
                
                // Store monitoring data
                app_state.add_monitoring_data(server.id.clone(), data);
                info!("✅ Server {} monitored successfully", server.name);
            }
            Err(e) => {
                warn!("⚠️ Failed to collect data for server {}: {}", server.name, e);
                
                // Update server status to error
                {
                    let mut servers = app_state.servers.write().unwrap();
                    if let Some(s) = servers.get_mut(&server.id) {
                        s.status = crate::models::ServerStatus::Error(e.to_string());
                        s.next_monitoring = chrono::Utc::now().timestamp() as u64 + s.monitoring_interval.as_secs();
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn get_cpu_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<CpuInfo> {
        // Try multiple commands for different Linux distributions
        let commands = vec![
            "cat /proc/stat | head -1",
            "top -bn1 | grep \"Cpu(s)\"",
            "vmstat 1 1 | tail -1",
        ];

        let mut cpu_usage = 0.0;
        let mut load_average = [0.0, 0.0, 0.0];
        let mut cores = 1;
        let mut found_cpu_data = false;

        for cmd in commands {
            if let Ok(output) = ssh_manager.execute_command(server, cmd).await {
                if let Ok(parsed) = Self::parse_cpu_usage(&output) {
                    cpu_usage = parsed;
                    found_cpu_data = true;
                    break;
                }
            }
        }

        if !found_cpu_data {
            return Err(anyhow::anyhow!("Failed to get CPU usage from any command"));
        }

        // Get load average
        if let Ok(output) = ssh_manager.execute_command(server, "cat /proc/loadavg").await {
            if let Ok(load) = Self::parse_load_average(&output) {
                load_average = load;
            }
        }

        // Get CPU cores
        if let Ok(output) = ssh_manager.execute_command(server, "nproc").await {
            cores = output.trim().parse().unwrap_or(1);
        }

        // Get CPU model
        let model = ssh_manager
            .execute_command(server, "cat /proc/cpuinfo | grep \"model name\" | head -1 | cut -d: -f2")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        Ok(CpuInfo {
            usage_percent: cpu_usage,
            load_average,
            cores: cores as u32,
            model,
        })
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

        // Try parsing from top command
        let re = Regex::new(r"Cpu\(s\):\s+(\d+\.?\d*)%us")?;
        if let Some(caps) = re.captures(output) {
            return Ok(caps.get(1).unwrap().as_str().parse()?);
        }

        Err(anyhow::anyhow!("Could not parse CPU usage"))
    }

    fn parse_load_average(output: &str) -> Result<[f64; 3]> {
        let parts: Vec<&str> = output.split_whitespace().collect();
        if parts.len() >= 3 {
            Ok([
                parts[0].parse()?,
                parts[1].parse()?,
                parts[2].parse()?,
            ])
        } else {
            Ok([0.0, 0.0, 0.0])
        }
    }

    async fn get_memory_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<MemoryInfo> {
        // Try /proc/meminfo first (Linux)
        if let Ok(output) = ssh_manager.execute_command(server, "cat /proc/meminfo").await {
            if let Ok(mem) = Self::parse_meminfo(&output) {
                return Ok(mem);
            }
        }

        // Try free command
        if let Ok(output) = ssh_manager.execute_command(server, "free -b").await {
            if let Ok(mem) = Self::parse_free_output(&output) {
                return Ok(mem);
            }
        }

        // If both commands failed, return an error
        Err(anyhow::anyhow!("Failed to get memory information from any command"))
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
                        "MemTotal:" => mem.total = value * 1024, // Convert from KB to bytes
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

    fn parse_free_output(output: &str) -> Result<MemoryInfo> {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() < 2 {
            return Err(anyhow::anyhow!("Invalid free command output"));
        }

        let mem_line: Vec<&str> = lines[1].split_whitespace().collect();
        let swap_line: Vec<&str> = lines[2].split_whitespace().collect();

        if mem_line.len() < 4 || swap_line.len() < 4 {
            return Err(anyhow::anyhow!("Invalid free command output"));
        }

        Ok(MemoryInfo {
            total: mem_line[1].parse()?,
            used: mem_line[2].parse()?,
            free: mem_line[3].parse()?,
            available: mem_line[6].parse().unwrap_or(0),
            swap_total: swap_line[1].parse()?,
            swap_used: swap_line[2].parse()?,
            swap_free: swap_line[3].parse()?,
        })
    }

    async fn get_disk_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<Vec<DiskInfo>> {
        // Try df command first
        if let Ok(output) = ssh_manager.execute_command(server, "df -h").await {
            if let Ok(disks) = Self::parse_df_output(&output) {
                return Ok(disks);
            }
        }

        // Try lsblk as fallback
        if let Ok(output) = ssh_manager.execute_command(server, "lsblk -f").await {
            if let Ok(disks) = Self::parse_lsblk_output(&output) {
                return Ok(disks);
            }
        }

        Ok(vec![])
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

    fn parse_lsblk_output(_output: &str) -> Result<Vec<DiskInfo>> {
        // This is a simplified parser for lsblk output
        // In a real implementation, you'd want to parse this more thoroughly
        Ok(vec![])
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

    async fn get_network_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<Vec<NetworkInfo>> {
        // Try /proc/net/dev first
        if let Ok(output) = ssh_manager.execute_command(server, "cat /proc/net/dev").await {
            if let Ok(networks) = Self::parse_net_dev(&output) {
                return Ok(networks);
            }
        }

        // Try ifconfig as fallback
        if let Ok(output) = ssh_manager.execute_command(server, "ifconfig").await {
            if let Ok(networks) = Self::parse_ifconfig(&output) {
                return Ok(networks);
            }
        }

        Ok(vec![])
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
                    let rx_bytes = stats[0].parse().unwrap_or(0);
                    let rx_packets = stats[1].parse().unwrap_or(0);
                    let rx_errors = stats[2].parse().unwrap_or(0);
                    let tx_bytes = stats[8].parse().unwrap_or(0);
                    let tx_packets = stats[9].parse().unwrap_or(0);
                    let tx_errors = stats[10].parse().unwrap_or(0);

                    networks.push(NetworkInfo {
                        interface,
                        rx_bytes,
                        tx_bytes,
                        rx_packets,
                        tx_packets,
                        rx_errors,
                        tx_errors,
                        ip_addresses: vec![], // Would need additional parsing
                    });
                }
            }
        }

        Ok(networks)
    }

    fn parse_ifconfig(_output: &str) -> Result<Vec<NetworkInfo>> {
        // Simplified ifconfig parser
        Ok(vec![])
    }

    async fn get_port_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<Vec<PortInfo>> {
        // Try netstat first
        if let Ok(output) = ssh_manager.execute_command(server, "netstat -tuln").await {
            if let Ok(ports) = Self::parse_netstat(&output) {
                return Ok(ports);
            }
        }

        // Try ss as fallback
        if let Ok(output) = ssh_manager.execute_command(server, "ss -tuln").await {
            if let Ok(ports) = Self::parse_ss(&output) {
                return Ok(ports);
            }
        }

        Ok(vec![])
    }

    fn parse_netstat(output: &str) -> Result<Vec<PortInfo>> {
        let mut ports = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for line in lines.iter().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let protocol = parts[0].to_lowercase();
                let local_address = parts[3];
                
                if let Some(port_str) = local_address.split(':').last() {
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

    fn parse_ss(_output: &str) -> Result<Vec<PortInfo>> {
        // Similar to netstat parsing
        Ok(vec![])
    }

    async fn get_system_info(ssh_manager: &SshConnectionManager, server: &Server) -> Result<SystemInfo> {
        let hostname = ssh_manager
            .execute_command(server, "hostname")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        let os = ssh_manager
            .execute_command(server, "uname -s")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        let kernel = ssh_manager
            .execute_command(server, "uname -r")
            .await
            .unwrap_or_default()
            .trim()
            .to_string();

        let uptime = ssh_manager
            .execute_command(server, "cat /proc/uptime")
            .await
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;

        let architecture = ssh_manager
            .execute_command(server, "uname -m")
            .await
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

    async fn run_ping_tests(ssh_manager: &SshConnectionManager, server: &Server) -> Result<Vec<PingTest>> {
        let targets = vec![
            "8.8.8.8",      // Google DNS
            "1.1.1.1",      // Cloudflare DNS
            "google.com",    // Google
            "github.com",    // GitHub
        ];

        let mut ping_tests = Vec::new();

        for target in targets {
            let ping_result = Self::ping_target(ssh_manager, server, target).await;
            ping_tests.push(ping_result);
        }

        Ok(ping_tests)
    }

    async fn ping_target(ssh_manager: &SshConnectionManager, server: &Server, target: &str) -> PingTest {
        let command = format!("ping -c 1 -W 5 {}", target);
        
        match ssh_manager.execute_command(server, &command).await {
            Ok(output) => {
                if let Some(latency) = Self::extract_ping_latency(&output) {
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
            Err(e) => PingTest {
                target: target.to_string(),
                latency_ms: None,
                success: false,
                error: Some(e.to_string()),
            },
        }
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
        use std::process::Command;
        
        let output = Command::new("cat")
            .arg("/proc/stat")
            .output()?;
        
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to read /proc/stat"));
        }
        
        let output_str = String::from_utf8(output.stdout)?;
        let lines: Vec<&str> = output_str.lines().collect();
        let cpu_line = lines.get(0).ok_or_else(|| anyhow::anyhow!("No CPU line found"))?;
        
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

            // Get load average
            let load_output = Command::new("cat").arg("/proc/loadavg").output()?;
            let load_str = String::from_utf8(load_output.stdout)?;
            let load_parts: Vec<&str> = load_str.split_whitespace().collect();
            let load_average = [
                load_parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                load_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                load_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
            ];

            // Get CPU cores
            let cores_output = Command::new("nproc").output()?;
            let cores = String::from_utf8(cores_output.stdout)?
                .trim()
                .parse()
                .unwrap_or(1);

            // Get CPU model
            let model_output = Command::new("cat")
                .arg("/proc/cpuinfo")
                .output()?;
            let model_str = String::from_utf8(model_output.stdout)?;
            let model = model_str
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
        use std::process::Command;
        
        let output = Command::new("cat").arg("/proc/meminfo").output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to read /proc/meminfo"));
        }
        
        let output_str = String::from_utf8(output.stdout)?;
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
        use std::process::Command;
        
        let output = Command::new("df")
            .arg("-h")
            .arg("--output=source,target,fstype,size,used,avail,pcent")
            .output()?;
        
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
        use std::process::Command;
        
        let output = Command::new("cat").arg("/proc/net/dev").output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to read /proc/net/dev"));
        }
        
        let output_str = String::from_utf8(output.stdout)?;
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
        use std::process::Command;
        
        let output = Command::new("ss")
            .arg("-tuln")
            .output()?;
        
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
                if let Some(port_str) = local_addr.split(':').last() {
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
        use std::process::Command;
        
        let hostname = Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        
        let os = Command::new("uname")
            .arg("-s")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        
        let kernel = Command::new("uname")
            .arg("-r")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_string();
        
        let uptime = Command::new("cat")
            .arg("/proc/uptime")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0) as u64;
        
        let architecture = Command::new("uname")
            .arg("-m")
            .output()
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
        
        let targets = vec![
            "8.8.8.8",
            "1.1.1.1",
            "google.com",
            "github.com",
        ];
        
        let mut ping_tests = Vec::new();
        
        for target in targets {
            let ping_result = Self::ping_local_target(target).await;
            ping_tests.push(ping_result);
        }
        
        Ok(ping_tests)
    }

    async fn ping_local_target(target: &str) -> PingTest {
        use std::process::Command;
        
        let command = format!("ping -c 1 -W 5 {}", target);
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output();
        
        match output {
            Ok(output) => {
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
            Err(e) => PingTest {
                target: target.to_string(),
                latency_ms: None,
                success: false,
                error: Some(e.to_string()),
            },
        }
    }
}
