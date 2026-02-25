use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringJob {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub job_type: JobType,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub metrics_collected: Vec<String>,
    pub retry_count: u32,
    pub priority: JobPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobType {
    FullCollection,
    CpuMemory,
    DiskNetwork,
    SystemInfo,
    PingTests,
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobType::FullCollection => write!(f, "Full Collection"),
            JobType::CpuMemory => write!(f, "CPU & Memory"),
            JobType::DiskNetwork => write!(f, "Disk & Network"),
            JobType::SystemInfo => write!(f, "System Info"),
            JobType::PingTests => write!(f, "Ping Tests"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
    Cancelled,
    Retrying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub proxy_config: Option<ProxyConfig>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub status: ServerStatus,
    pub monitoring_interval: Duration,
    pub next_monitoring: u64, // Unix timestamp for next monitoring
    pub connection_id: Option<String>, // For persistent connections
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    SshConfig, // Use default SSH config
    Password(String), // For servers that need password authentication
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub ssh_config_path: String,
    pub fallback_password: Option<String>, // Fallback password for SSH connections
    pub connection_timeout: Duration,
    pub keep_alive_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_username: Option<String>,
    pub proxy_auth: Option<AuthMethod>,
    pub chain: Option<Box<ProxyConfig>>, // For SSH chaining
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProxyType {
    JumpHost,
    Tunnel,
    Chain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerStatus {
    Online,
    Offline,
    Connecting,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringData {
    pub server_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: Vec<NetworkInfo>,
    pub ports: Vec<PortInfo>,
    pub ping_tests: Vec<PingTest>,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub usage_percent: f64,
    pub load_average: [f64; 3],
    pub cores: u32,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_free: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub device: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f64,
    pub filesystem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub ip_addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String,
    pub state: String,
    pub process: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingTest {
    pub target: String,
    pub latency_ms: Option<f64>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime: u64,
    pub architecture: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub servers: Arc<RwLock<HashMap<String, Server>>>,
    pub monitoring_data: Arc<RwLock<HashMap<String, Vec<MonitoringData>>>>,
    pub server_config: Arc<RwLock<ServerConfig>>,
    pub ssh_connections: Arc<RwLock<HashMap<String, SshConnectionInfo>>>,
    pub jobs: Arc<RwLock<Vec<MonitoringJob>>>,
    pub paused_servers: Arc<RwLock<std::collections::HashSet<String>>>,
}

#[derive(Debug, Clone)]
pub struct SshConnectionInfo {
    pub server_id: String,
    pub connection_id: String,
    pub last_used: u64, // Unix timestamp
    pub is_active: bool,
}

impl AppState {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        // Find SSH config path
        let ssh_config_path = Self::find_ssh_config_path().await?;
        
        let mut servers = HashMap::new();
        
        // Add local machine as first server
        let now = chrono::Utc::now();
        let local_server = Server {
            id: "local".to_string(),
            name: "Local Machine".to_string(),
            host: "localhost".to_string(),
            port: 22,
            username: whoami::username(),
            auth_method: AuthMethod::SshConfig,
            status: ServerStatus::Online,
            last_seen: Some(now),
            next_monitoring: now.timestamp() as u64,
            monitoring_interval: Duration::from_secs(3), // 3-second updates for local
            proxy_config: None,
            created_at: now,
            updated_at: now,
            connection_id: None,
        };
        servers.insert("local".to_string(), local_server);
        
        Ok(Self {
            servers: Arc::new(RwLock::new(servers)),
            monitoring_data: Arc::new(RwLock::new(HashMap::new())),
            server_config: Arc::new(RwLock::new(ServerConfig {
                ssh_config_path,
                fallback_password: config.fallback_password,
                connection_timeout: Duration::from_secs(10),
                keep_alive_interval: Duration::from_secs(30),
            })),
            ssh_connections: Arc::new(RwLock::new(HashMap::new())),
            jobs: Arc::new(RwLock::new(Vec::new())),
            paused_servers: Arc::new(RwLock::new(std::collections::HashSet::new())),
        })
    }

    async fn find_ssh_config_path() -> anyhow::Result<String> {
        // Try to find SSH config using ssh command
        let output = tokio::process::Command::new("ssh")
            .args(["-F", "/dev/null", "-G", "localhost"])
            .output()
            .await?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.starts_with("userknownhostsfile") {
                    if let Some(path) = line.split_whitespace().nth(1) {
                        if let Some(config_path) = path.strip_suffix("/known_hosts") {
                            return Ok(format!("{}/config", config_path));
                        }
                    }
                }
            }
        }
        
        // Fallback to default path
        Ok(format!("{}/.ssh/config", std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())))
    }

    pub fn add_monitoring_data(&self, server_id: String, data: MonitoringData) {
        let mut monitoring_data = self.monitoring_data.write().unwrap();
        let server_data = monitoring_data.entry(server_id).or_insert_with(Vec::new);
        server_data.push(data);
        
        // Keep only last 1000 entries per server for historical records
        if server_data.len() > 1000 {
            server_data.drain(0..server_data.len() - 1000);
        }
    }

    pub fn get_latest_monitoring_data(&self, server_id: &str) -> Option<MonitoringData> {
        let monitoring_data = self.monitoring_data.read().unwrap();
        monitoring_data.get(server_id).and_then(|data| data.last().cloned())
    }

    pub fn get_historical_data(&self, server_id: &str, limit: usize) -> Vec<MonitoringData> {
        let monitoring_data = self.monitoring_data.read().unwrap();
        if let Some(data) = monitoring_data.get(server_id) {
            data.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub async fn load_servers_from_ssh_config(&self) -> anyhow::Result<()> {
        let config = self.server_config.read().unwrap();
        let ssh_config_path = &config.ssh_config_path;
        
        // Parse SSH config file
        let hosts = Self::parse_ssh_config(ssh_config_path).await?;
        
        let mut servers = self.servers.write().unwrap();
        // Don't clear existing servers - keep the local machine
        
        for (i, host) in hosts.iter().enumerate() {
            // Skip hosts with empty hostnames or usernames
            if host.host.is_empty() || host.username.is_empty() {
                warn!("⚠️ Skipping host '{}' - missing hostname or username", host.name);
                continue;
            }
            
            let server = Server {
                id: host.name.clone(),
                name: host.name.clone(),
                host: host.host.clone(),
                port: host.port,
                username: host.username.clone(),
                auth_method: AuthMethod::SshConfig,
                proxy_config: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                last_seen: None,
                status: ServerStatus::Offline,
                monitoring_interval: Duration::from_secs(30),
                next_monitoring: (chrono::Utc::now().timestamp() as u64) + (i as u64 * 5), // Stagger monitoring
                connection_id: None,
            };
            servers.insert(server.id.clone(), server);
        }
        
        Ok(())
    }

    async fn parse_ssh_config(path: &str) -> anyhow::Result<Vec<SshHost>> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut hosts = Vec::new();
        let mut current_host: Option<SshHost> = None;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if line.starts_with("Host ") {
                if let Some(mut host) = current_host.take() {
                    // Apply SSH config defaults
                    if host.host.is_empty() {
                        host.host = host.name.clone(); // HostName defaults to Host
                    }
                    if host.username.is_empty() {
                        host.username = whoami::username(); // User defaults to current user
                    }
                    // Port already defaults to 22
                    hosts.push(host);
                }
                let host_name = line.strip_prefix("Host ").unwrap().trim();
                current_host = Some(SshHost {
                    name: host_name.to_string(),
                    host: String::new(),
                    port: 22, // Default SSH port
                    username: whoami::username(), // Default to current user
                });
            } else if let Some(host) = &mut current_host {
                if line.starts_with("HostName ") {
                    host.host = line.strip_prefix("HostName ").unwrap().trim().to_string();
                } else if line.starts_with("Port ") {
                    if let Ok(port) = line.strip_prefix("Port ").unwrap().trim().parse::<u16>() {
                        host.port = port;
                    }
                } else if line.starts_with("User ") {
                    host.username = line.strip_prefix("User ").unwrap().trim().to_string();
                }
            }
        }
        
        if let Some(mut host) = current_host {
            // Apply SSH config defaults
            if host.host.is_empty() {
                host.host = host.name.clone(); // HostName defaults to Host
            }
            if host.username.is_empty() {
                host.username = whoami::username(); // User defaults to current user
            }
            // Port already defaults to 22
            hosts.push(host);
        }
        
        Ok(hosts)
    }

    pub fn get_connection_id(&self, server_id: &str) -> Option<String> {
        let connections = self.ssh_connections.read().unwrap();
        connections.get(server_id).map(|conn| conn.connection_id.clone())
    }

    pub fn set_connection_id(&self, server_id: String, connection_id: String) {
        let mut connections = self.ssh_connections.write().unwrap();
        connections.insert(server_id.clone(), SshConnectionInfo {
            server_id,
            connection_id,
            last_used: chrono::Utc::now().timestamp() as u64,
            is_active: true,
        });
    }

    pub fn update_connection_usage(&self, server_id: &str) {
        let mut connections = self.ssh_connections.write().unwrap();
        if let Some(conn) = connections.get_mut(server_id) {
            conn.last_used = chrono::Utc::now().timestamp() as u64;
        }
    }

    pub fn mark_connection_inactive(&self, server_id: &str) {
        let mut connections = self.ssh_connections.write().unwrap();
        if let Some(conn) = connections.get_mut(server_id) {
            conn.is_active = false;
        }
    }

    pub fn add_job(&self, job: MonitoringJob) {
        let mut jobs = self.jobs.write().unwrap();
        jobs.push(job);
        // Keep only the last 200 jobs
        let len = jobs.len();
        if len > 200 {
            jobs.drain(0..len - 200);
        }
    }

    pub fn update_job_status(&self, job_id: &str, status: JobStatus, error: Option<String>, duration_ms: Option<u64>, metrics: Option<Vec<String>>) {
        let mut jobs = self.jobs.write().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            job.status = status.clone();
            if let Some(e) = error {
                job.error = Some(e);
            }
            if let Some(d) = duration_ms {
                job.duration_ms = Some(d);
            }
            if let Some(m) = metrics {
                job.metrics_collected = m;
            }
            if status == JobStatus::Running {
                job.started_at = Some(Utc::now());
            }
            if status == JobStatus::Completed || status == JobStatus::Failed {
                job.completed_at = Some(Utc::now());
            }
        }
    }

    pub fn get_jobs(&self, limit: usize) -> Vec<MonitoringJob> {
        let jobs = self.jobs.read().unwrap();
        jobs.iter().rev().take(limit).cloned().collect()
    }

    pub fn get_jobs_filtered(&self, limit: usize, status_filter: Option<&str>, server_filter: Option<&str>) -> Vec<MonitoringJob> {
        let jobs = self.jobs.read().unwrap();
        jobs.iter()
            .rev()
            .filter(|j| {
                if let Some(sf) = status_filter {
                    let job_status = format!("{:?}", j.status);
                    if !job_status.eq_ignore_ascii_case(sf) {
                        return false;
                    }
                }
                if let Some(svf) = server_filter {
                    if j.server_id != svf {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn get_job_statistics(&self) -> JobStatistics {
        let jobs = self.jobs.read().unwrap();
        let total = jobs.len();
        let completed = jobs.iter().filter(|j| j.status == JobStatus::Completed).count();
        let failed = jobs.iter().filter(|j| j.status == JobStatus::Failed).count();
        let running = jobs.iter().filter(|j| j.status == JobStatus::Running).count();
        let pending = jobs.iter().filter(|j| j.status == JobStatus::Pending).count();
        let cancelled = jobs.iter().filter(|j| j.status == JobStatus::Cancelled).count();

        let avg_duration = {
            let completed_jobs: Vec<&MonitoringJob> = jobs.iter().filter(|j| j.status == JobStatus::Completed && j.duration_ms.is_some()).collect();
            if completed_jobs.is_empty() {
                0.0
            } else {
                let total_duration: u64 = completed_jobs.iter().map(|j| j.duration_ms.unwrap_or(0)).sum();
                total_duration as f64 / completed_jobs.len() as f64
            }
        };

        let success_rate = if completed + failed > 0 {
            (completed as f64 / (completed + failed) as f64) * 100.0
        } else {
            0.0
        };

        JobStatistics {
            total,
            completed,
            failed,
            running,
            pending,
            cancelled,
            avg_duration_ms: avg_duration,
            success_rate,
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.write().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
            if job.status == JobStatus::Pending || job.status == JobStatus::Running {
                job.status = JobStatus::Cancelled;
                job.completed_at = Some(Utc::now());
                return true;
            }
        }
        false
    }

    pub fn clear_completed_jobs(&self) -> usize {
        let mut jobs = self.jobs.write().unwrap();
        let before = jobs.len();
        jobs.retain(|j| j.status != JobStatus::Completed && j.status != JobStatus::Cancelled);
        before - jobs.len()
    }

    pub fn clear_failed_jobs(&self) -> usize {
        let mut jobs = self.jobs.write().unwrap();
        let before = jobs.len();
        jobs.retain(|j| j.status != JobStatus::Failed);
        before - jobs.len()
    }

    pub fn clear_all_jobs(&self) -> usize {
        let mut jobs = self.jobs.write().unwrap();
        let count = jobs.len();
        // Only clear non-running jobs
        jobs.retain(|j| j.status == JobStatus::Running);
        count - jobs.len()
    }

    pub fn is_server_paused(&self, server_id: &str) -> bool {
        let paused = self.paused_servers.read().unwrap();
        paused.contains(server_id)
    }

    pub fn pause_server(&self, server_id: &str) {
        let mut paused = self.paused_servers.write().unwrap();
        paused.insert(server_id.to_string());
    }

    pub fn resume_server(&self, server_id: &str) {
        let mut paused = self.paused_servers.write().unwrap();
        paused.remove(server_id);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatistics {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub running: usize,
    pub pending: usize,
    pub cancelled: usize,
    pub avg_duration_ms: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
struct SshHost {
    name: String,
    host: String,
    port: u16,
    username: String,
}


