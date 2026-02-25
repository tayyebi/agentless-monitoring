use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

use crate::models::{AppState, AuthMethod, ProxyConfig, Server};

pub struct SshConnection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub fallback_password: Option<String>,
}

pub struct SshConnectionManager {
    connections: Arc<RwLock<HashMap<String, SshConnectionInfo>>>,
    app_state: Arc<AppState>,
    max_connections: usize,
}

#[derive(Debug)]
struct SshConnectionInfo {
    pub process: Option<std::process::Child>,
}

impl SshConnection {
    pub async fn new(server: &Server) -> Result<Self> {
        Ok(Self {
            host: server.host.clone(),
            port: server.port,
            username: server.username.clone(),
            auth_method: server.auth_method.clone(),
            fallback_password: None,
        })
    }

    pub async fn new_with_fallback(
        server: &Server,
        fallback_password: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            host: server.host.clone(),
            port: server.port,
            username: server.username.clone(),
            auth_method: server.auth_method.clone(),
            fallback_password,
        })
    }

    pub async fn new_with_proxy(server: &Server, _proxy_config: &ProxyConfig) -> Result<Self> {
        // For now, we'll implement a simplified version
        // In a production system, you'd want to use a proper SSH library
        Self::new(server).await
    }

    pub async fn execute_command(&self, command: &str) -> Result<String> {
        // Try primary authentication method first
        let result = self.try_execute_command(command).await;

        // If it fails and we have a fallback password, try with fallback
        if result.is_err() && self.fallback_password.is_some() {
            let fallback_connection = SshConnection {
                host: self.host.clone(),
                port: self.port,
                username: self.username.clone(),
                auth_method: AuthMethod::Password(self.fallback_password.clone().unwrap()),
                fallback_password: None,
            };
            return fallback_connection.try_execute_command(command).await;
        }

        result
    }

    async fn try_execute_command(&self, command: &str) -> Result<String> {
        let ssh_args = self.build_ssh_args();
        let username = self.username.clone();
        let host = self.host.clone();
        let command = command.to_string();

        let output = timeout(
            Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                let command_for_log = command.clone();
                info!(
                    "🔍 Executing SSH command: ssh {} {}@{} \"{}\"",
                    ssh_args.join(" "),
                    username,
                    host,
                    command_for_log
                );
                info!(
                    "🔍 Full command: ssh {} {}@{} \"{}\"",
                    ssh_args.join(" "),
                    username,
                    host,
                    command_for_log
                );
                let mut cmd = Command::new("ssh");
                cmd.args(&ssh_args)
                    .arg(format!("{}@{}", username, host))
                    .arg(command);
                cmd.output()
            }),
        )
        .await??;

        let output = output?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check for common password/authentication error patterns
            if stderr.contains("Permission denied")
                || stderr.contains("password")
                || stderr.contains("authentication")
                || stderr.contains("passphrase")
                || stderr.contains("Host key verification failed")
            {
                warn!(
                    "🔐 SSH authentication failed for {}@{}: {}",
                    self.username, self.host, stderr
                );
                return Err(anyhow::anyhow!("SSH authentication failed: {}", stderr));
            }
            error!(
                "💥 SSH command failed for {}@{}: {}",
                self.username, self.host, stderr
            );
            return Err(anyhow::anyhow!("SSH command failed: {}", stderr));
        }

        Ok(String::from_utf8(output.stdout)?)
    }

    fn build_ssh_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "LogLevel=ERROR".to_string(),
            "-p".to_string(),
            self.port.to_string(),
        ];

        match &self.auth_method {
            AuthMethod::Password(password) => {
                // For password auth, we'll use sshpass
                args.insert(0, "sshpass".to_string());
                args.insert(1, "-p".to_string());
                args.insert(2, password.clone());
            }
            AuthMethod::SshConfig => {
                // Use default SSH config, no additional args needed
                // SSH will automatically use ~/.ssh/config
            }
        }

        args
    }
}

impl SshConnectionManager {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            app_state,
            max_connections: 50, // Maximum number of concurrent SSH connections
        }
    }

    pub async fn get_or_create_connection(&self, server: &Server) -> Result<String> {
        let server_id = server.id.clone();

        // Check if we already have an active connection
        if let Some(conn_id) = self.app_state.get_connection_id(&server_id) {
            if self.is_connection_active(&conn_id).await {
                self.app_state.update_connection_usage(&server_id);
                return Ok(conn_id);
            }
        }

        // Check connection pool size limit
        let should_cleanup = {
            let connections = self.connections.read().unwrap();
            connections.len() >= self.max_connections
        };

        if should_cleanup {
            // Clean up inactive connections first
            self.cleanup_inactive_connections().await;

            // Check again after cleanup
            let connections = self.connections.read().unwrap();
            if connections.len() >= self.max_connections {
                return Err(anyhow::anyhow!("Connection pool is full"));
            }
        }

        // Create new connection
        let connection_id = uuid::Uuid::new_v4().to_string();
        let fallback_password = {
            let config = self.app_state.server_config.read().unwrap();
            config.fallback_password.clone()
        };
        let ssh_conn = SshConnection::new_with_fallback(server, fallback_password).await?;

        // Start persistent SSH connection
        self.start_persistent_connection(&connection_id, &server_id, &ssh_conn)
            .await?;

        // Store connection info
        self.app_state
            .set_connection_id(server_id.clone(), connection_id.clone());

        info!("🔗 Created new SSH connection for server: {}", server_id);
        Ok(connection_id)
    }

    async fn start_persistent_connection(
        &self,
        connection_id: &str,
        _server_id: &str,
        ssh_conn: &SshConnection,
    ) -> Result<()> {
        let ssh_args = ssh_conn.build_ssh_args();
        let username = ssh_conn.username.clone();
        let host = ssh_conn.host.clone();

        // Start SSH connection with ControlMaster
        let mut cmd = Command::new("ssh");
        cmd.args(&ssh_args)
            .arg("-M") // ControlMaster
            .arg("-S") // ControlPath
            .arg(format!("/tmp/ssh_control_{}", connection_id))
            .arg("-N") // No command execution
            .arg(format!("{}@{}", username, host))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let process = cmd.spawn()?;

        // Store connection info
        let mut connections = self.connections.write().unwrap();
        connections.insert(
            connection_id.to_string(),
            SshConnectionInfo {
                process: Some(process),
            },
        );

        Ok(())
    }

    async fn is_connection_active(&self, connection_id: &str) -> bool {
        let mut connections = self.connections.write().unwrap();
        if let Some(conn) = connections.get_mut(connection_id) {
            if let Some(ref mut process) = conn.process {
                // Check if process is still running
                match process.try_wait() {
                    Ok(Some(_)) => false, // Process has exited
                    Ok(None) => true,     // Process is still running
                    Err(_) => false,      // Error checking process
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    pub async fn execute_command(&self, server: &Server, command: &str) -> Result<String> {
        let connection_id = self.get_or_create_connection(server).await?;

        // Execute command using the persistent ControlMaster connection
        let control_path = format!("/tmp/ssh_control_{}", connection_id);
        let username = server.username.clone();
        let host = server.host.clone();
        let command = command.to_string();

        let output = timeout(
            Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                let command_for_log = command.clone();
                info!(
                    "🔍 Executing SSH command: ssh -S {} {}@{} \"{}\"",
                    control_path, username, host, command_for_log
                );
                // Execute command through the persistent connection
                let mut cmd = Command::new("ssh");
                cmd.arg("-S") // ControlPath
                    .arg(&control_path)
                    .arg("-q") // Quiet mode
                    .arg(format!("{}@{}", username, host))
                    .arg(command);
                cmd.output()
            }),
        )
        .await??;

        let output = output?;

        if !output.status.success() {
            // Connection might be dead, mark it as inactive
            self.app_state.mark_connection_inactive(&server.id);
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check for common password/authentication error patterns
            if stderr.contains("Permission denied")
                || stderr.contains("password")
                || stderr.contains("authentication")
                || stderr.contains("passphrase")
                || stderr.contains("Host key verification failed")
            {
                warn!(
                    "🔐 SSH authentication failed for {}@{}: {}",
                    server.username, server.host, stderr
                );
                return Err(anyhow::anyhow!("SSH authentication failed: {}", stderr));
            }
            error!(
                "💥 SSH command failed for {}@{}: {}",
                server.username, server.host, stderr
            );
            return Err(anyhow::anyhow!("SSH command failed: {}", stderr));
        }

        // Update last used time
        self.app_state.update_connection_usage(&server.id);

        Ok(String::from_utf8(output.stdout)?)
    }

    pub async fn cleanup_inactive_connections(&self) {
        let mut connections = self.connections.write().unwrap();
        let mut to_remove = Vec::new();

        for (conn_id, conn_info) in connections.iter_mut() {
            if let Some(ref mut process) = conn_info.process {
                if let Ok(Some(_)) = process.try_wait() {
                    // Process has exited
                    to_remove.push(conn_id.clone());
                }
            }
        }

        for conn_id in to_remove {
            connections.remove(&conn_id);
        }
    }
}
