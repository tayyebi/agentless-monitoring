use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::time::{timeout, Duration};
use tracing::{error, info, warn};

use crate::models::{AppState, AuthMethod, ProxyConfig, Server};

/// Polling interval while waiting for the ControlMaster socket to appear.
const CONTROL_SOCKET_POLL_MS: u64 = 200;

/// Timeout for `ssh -O check` health probes (seconds).
const CONTROL_CHECK_TIMEOUT_SECS: u64 = 5;

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
    pub server_id: String,
    pub username: String,
    pub host: String,
}

/// Returns true if the error string indicates a broken or lost SSH connection
/// (as opposed to an authentication failure or a remote command error).
fn is_connection_error(msg: &str) -> bool {
    msg.contains("ControlSocket")
        || msg.contains("Broken pipe")
        || msg.contains("Connection reset")
        || msg.contains("mux")
        || msg.contains("Connection refused")
        || msg.contains("No route to host")
        || msg.contains("Connection timed out")
        || msg.contains("ssh_exchange_identification")
        || msg.contains("kex_exchange_identification")
}

/// Return the directory used to store ControlMaster sockets.
///
/// Preference order:
///   1. `$XDG_RUNTIME_DIR/agentless-monitor`  (user-specific, mode 700 on most distros)
///   2. `$HOME/.ssh/control`                   (typically mode 700)
///
/// The directory is created with mode 0700 on first call if it does not exist.
fn control_socket_dir() -> String {
    let dir = if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{}/agentless-monitor", runtime)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        format!("{}/.ssh/control", home)
    };

    if !std::path::Path::new(&dir).exists() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warn!("⚠️ Could not create control socket directory {}: {}", dir, e);
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &dir,
                    std::fs::Permissions::from_mode(0o700),
                );
            }
        }
    }

    dir
}

/// Build the full path for a ControlMaster socket for `connection_id`.
fn control_socket_path(connection_id: &str) -> String {
    format!("{}/ssh_{}", control_socket_dir(), connection_id)
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

    // ── Synchronous helpers (no lock guard crosses an await point) ──────────

    /// Insert a new connection record. Returns `false` if the pool is full.
    fn store_connection(&self, connection_id: &str, info: SshConnectionInfo) {
        let mut connections = self.connections.write().unwrap();
        connections.insert(connection_id.to_string(), info);
    }

    /// Check whether the ControlMaster process for `connection_id` is still
    /// running.  Takes a write lock momentarily so it can call `try_wait`.
    fn is_process_running(&self, connection_id: &str) -> bool {
        let mut connections = self.connections.write().unwrap();
        if let Some(conn) = connections.get_mut(connection_id) {
            if let Some(ref mut process) = conn.process {
                return matches!(process.try_wait(), Ok(None));
            }
        }
        false
    }

    /// Return the (username, host) pair stored for `connection_id`, if any.
    fn get_connection_hosts(&self, connection_id: &str) -> Option<(String, String)> {
        let connections = self.connections.read().unwrap();
        connections
            .get(connection_id)
            .map(|c| (c.username.clone(), c.host.clone()))
    }

    /// Return the current pool size.
    fn pool_size(&self) -> usize {
        self.connections.read().unwrap().len()
    }

    /// Remove a connection from the local map and return its child process (if
    /// any) so the caller can kill it outside the lock.
    fn take_connection(&self, connection_id: &str) -> Option<std::process::Child> {
        let mut connections = self.connections.write().unwrap();
        connections
            .remove(connection_id)
            .and_then(|mut info| info.process.take())
    }

    /// Collect the IDs of every connection whose ControlMaster process has
    /// already exited.  Returns `(connection_id, server_id)` pairs.
    fn find_dead_connections(&self) -> Vec<(String, String)> {
        let mut connections = self.connections.write().unwrap();
        connections
            .iter_mut()
            .filter_map(|(conn_id, info)| {
                if let Some(ref mut proc) = info.process {
                    if let Ok(Some(_)) = proc.try_wait() {
                        return Some((conn_id.clone(), info.server_id.clone()));
                    }
                }
                None
            })
            .collect()
    }

    // ── Async methods (never hold a lock guard across `.await`) ─────────────

    pub async fn get_or_create_connection(&self, server: &Server) -> Result<String> {
        let server_id = server.id.clone();

        // Check if we already have an active connection
        if let Some(conn_id) = self.app_state.get_connection_id(&server_id) {
            if self.is_connection_active(&conn_id).await {
                self.app_state.update_connection_usage(&server_id);
                return Ok(conn_id);
            }
            // Connection is dead; clean it up before creating a new one
            self.remove_connection(&conn_id, &server_id).await;
        }

        // Check connection pool size limit
        if self.pool_size() >= self.max_connections {
            // Clean up inactive connections first
            self.cleanup_inactive_connections().await;

            if self.pool_size() >= self.max_connections {
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
        server_id: &str,
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
            .arg(control_socket_path(connection_id))
            .arg("-N") // No command execution
            .arg(format!("{}@{}", username, host))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let process = cmd.spawn()?;

        // Store connection info via the synchronous helper (no lock held past here).
        self.store_connection(
            connection_id,
            SshConnectionInfo {
                process: Some(process),
                server_id: server_id.to_string(),
                username: username.clone(),
                host: host.clone(),
            },
        );

        // Wait for the control socket to appear so the connection is usable
        // immediately after this function returns.  Bail out early if the
        // ControlMaster process exits before the socket is ready (e.g. auth
        // failure).
        let control_path = control_socket_path(connection_id);
        let connection_id_owned = connection_id.to_string();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            // Use the synchronous helper so no lock guard crosses the await.
            if !self.is_process_running(&connection_id_owned) {
                return Err(anyhow::anyhow!(
                    "SSH ControlMaster process exited prematurely for {}@{}",
                    username,
                    host
                ));
            }

            if std::path::Path::new(&control_path).exists() {
                info!(
                    "🔗 ControlMaster socket ready for {}@{}: {}",
                    username, host, control_path
                );
                break;
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timed out waiting for SSH ControlMaster socket for {}@{}",
                    username,
                    host
                ));
            }

            tokio::time::sleep(Duration::from_millis(CONTROL_SOCKET_POLL_MS)).await;
        }

        Ok(())
    }

    async fn is_connection_active(&self, connection_id: &str) -> bool {
        // 1. Check if the ControlMaster process is still running (synchronous helper).
        if !self.is_process_running(connection_id) {
            return false;
        }

        // 2. Fast path: verify the control socket file exists.
        let control_path = control_socket_path(connection_id);
        if !std::path::Path::new(&control_path).exists() {
            return false;
        }

        // 3. Verify the socket is actually responsive via `ssh -O check`.
        //    Use the synchronous helper so no lock guard crosses the await.
        let (username, host) = match self.get_connection_hosts(connection_id) {
            Some(h) => h,
            None => return false,
        };

        match timeout(
            Duration::from_secs(CONTROL_CHECK_TIMEOUT_SECS),
            tokio::task::spawn_blocking(move || {
                Command::new("ssh")
                    .arg("-S")
                    .arg(&control_path)
                    .arg("-O")
                    .arg("check")
                    .arg(format!("{}@{}", username, host))
                    .output()
            }),
        )
        .await
        {
            Ok(Ok(Ok(output))) => output.status.success(),
            _ => false,
        }
    }

    /// Remove a connection: kill the ControlMaster process, delete the socket
    /// file, and mark it inactive in AppState.
    async fn remove_connection(&self, connection_id: &str, server_id: &str) {
        // Extract the process via the synchronous helper (no lock held past here).
        if let Some(mut proc) = self.take_connection(connection_id) {
            let _ = proc.kill();
        }

        // Remove the control socket file so future checks see a clean state.
        let control_path = control_socket_path(connection_id);
        let _ = std::fs::remove_file(&control_path);

        self.app_state.mark_connection_inactive(server_id);
    }

    /// Execute a command through an already-established ControlMaster connection.
    async fn run_command_through_connection(
        &self,
        connection_id: &str,
        server: &Server,
        command: &str,
    ) -> Result<String> {
        let control_path = control_socket_path(connection_id);
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

        Ok(String::from_utf8(output.stdout)?)
    }

    pub async fn execute_command(&self, server: &Server, command: &str) -> Result<String> {
        let connection_id = self.get_or_create_connection(server).await?;

        match self
            .run_command_through_connection(&connection_id, server, command)
            .await
        {
            Ok(output) => {
                self.app_state.update_connection_usage(&server.id);
                Ok(output)
            }
            Err(e) if is_connection_error(&e.to_string()) => {
                // Connection is broken – clean it up and retry once with a fresh connection.
                warn!(
                    "🔄 SSH connection broken for {}, reconnecting: {}",
                    server.id, e
                );
                self.remove_connection(&connection_id, &server.id).await;

                let new_connection_id = self.get_or_create_connection(server).await?;
                let result = self
                    .run_command_through_connection(&new_connection_id, server, command)
                    .await?;
                self.app_state.update_connection_usage(&server.id);
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn cleanup_inactive_connections(&self) {
        // Collect dead connections via the synchronous helper (no lock held past here).
        for (conn_id, server_id) in self.find_dead_connections() {
            self.remove_connection(&conn_id, &server_id).await;
        }
    }
}
