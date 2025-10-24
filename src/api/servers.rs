use anyhow::Result;
use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;
use std::collections::HashMap;

use crate::models::{AppState, Server, ServerStatus};
use crate::ssh::{SshConnection, SshConnectionManager};
use crate::monitoring::MonitoringService;

pub async fn list_servers(State(state): State<std::sync::Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let servers = state.servers.read().unwrap();
    let mut servers: Vec<Server> = servers.values().cloned().collect();
    
    // Sort servers: local machine first, then others in creation order
    servers.sort_by(|a, b| {
        if a.id == "local" {
            std::cmp::Ordering::Less
        } else if b.id == "local" {
            std::cmp::Ordering::Greater
        } else {
            // For non-local servers, sort by creation time (which preserves SSH config order)
            a.created_at.cmp(&b.created_at)
        }
    });
    
    Ok(Json(json!(servers)))
}

pub async fn create_server(
    State(state): State<std::sync::Arc<AppState>>,
    Json(server): Json<CreateServerRequest>,
) -> Result<Json<Value>, StatusCode> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let server = Server {
        id: id.clone(),
        name: server.name,
        host: server.host,
        port: server.port,
        username: server.username,
        auth_method: server.auth_method,
        proxy_config: server.proxy_config,
        created_at: now,
        updated_at: now,
        last_seen: None,
        status: ServerStatus::Offline,
        monitoring_interval: std::time::Duration::from_secs(30),
        next_monitoring: chrono::Utc::now().timestamp() as u64,
        connection_id: None,
    };

    {
        let mut servers = state.servers.write().unwrap();
        servers.insert(id.clone(), server);
    }

    Ok(Json(json!({
        "id": id,
        "message": "Server created successfully"
    })))
}

pub async fn get_server(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let servers = state.servers.read().unwrap();
    match servers.get(&id) {
        Some(server) => Ok(Json(json!(server))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn update_server(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<UpdateServerRequest>,
) -> Result<Json<Value>, StatusCode> {
    let now = chrono::Utc::now();

    {
        let mut servers = state.servers.write().unwrap();
        if let Some(server) = servers.get_mut(&id) {
            server.name = update.name;
            server.host = update.host;
            server.port = update.port;
            server.username = update.username;
            server.auth_method = update.auth_method;
            server.proxy_config = update.proxy_config;
            server.updated_at = now;
        } else {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    Ok(Json(json!({
        "message": "Server updated successfully"
    })))
}

pub async fn delete_server(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    {
        let mut servers = state.servers.write().unwrap();
        if servers.remove(&id).is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    Ok(Json(json!({
        "message": "Server deleted successfully"
    })))
}

#[derive(Deserialize)]
pub struct ConnectRequest {
    password: Option<String>,
}

pub async fn connect_server(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<ConnectRequest>,
) -> Result<Json<Value>, StatusCode> {
    let server = {
        let servers = state.servers.read().unwrap();
        servers.get(&id).cloned()
    };

    let server = match server {
        Some(server) => server,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Use provided password or fallback password from config
    let password = request.password.or_else(|| {
        let server_config = state.server_config.read().unwrap();
        server_config.fallback_password.clone()
    });

    // Try to establish SSH connection
    let connection_result = if let Some(proxy_config) = &server.proxy_config {
        SshConnection::new_with_proxy(&server, proxy_config).await
    } else {
        SshConnection::new_with_fallback(&server, password).await
    };

    match connection_result {
        Ok(connection) => {
            // Test connection with a simple command
            match connection.execute_command("echo \"test\"").await {
                Ok(_) => {
                    // Update server status to online
                    {
                        let mut servers = state.servers.write().unwrap();
                        if let Some(server) = servers.get_mut(&id) {
                            server.status = ServerStatus::Online;
                            server.last_seen = Some(chrono::Utc::now());
                        }
                    }

                    // Connection successful

                    Ok(Json(json!({
                        "status": "connected",
                        "message": "Successfully connected to server"
                    })))
                }
                Err(e) => {
                    // Update server status to error
                    {
                        let mut servers = state.servers.write().unwrap();
                        if let Some(server) = servers.get_mut(&id) {
                            server.status = ServerStatus::Error(e.to_string());
                        }
                    }

                    Ok(Json(json!({
                        "status": "error",
                        "message": e.to_string()
                    })))
                }
            }
        }
        Err(e) => {
            // Update server status to error
            {
                let mut servers = state.servers.write().unwrap();
                if let Some(server) = servers.get_mut(&id) {
                    server.status = ServerStatus::Error(e.to_string());
                }
            }

            Ok(Json(json!({
                "status": "error",
                "message": e.to_string()
            })))
        }
    }
}

pub async fn monitor_server(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Get server details
    let server = {
        let servers = state.servers.read().unwrap();
        servers.get(&id).cloned()
    };

    let server = match server {
        Some(server) => server,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Get fallback password from config
    let fallback_password = {
        let server_config = state.server_config.read().unwrap();
        server_config.fallback_password.clone()
    };

    // Create SSH connection
    let connection = if let Some(proxy_config) = &server.proxy_config {
        SshConnection::new_with_proxy(&server, proxy_config).await
    } else {
        SshConnection::new_with_fallback(&server, fallback_password).await
    };

    match connection {
        Ok(_conn) => {
            let ssh_manager = SshConnectionManager::new(state.clone());
            match MonitoringService::collect_data(&ssh_manager, &server).await {
                Ok(mut data) => {
                    // Store monitoring data
                    data.server_id = id.clone();
                    state.add_monitoring_data(id.clone(), data.clone());

                    Ok(Json(json!(data)))
                }
                Err(e) => {
                    Ok(Json(json!({
                        "error": e.to_string()
                    })))
                }
            }
        }
        Err(e) => {
            Ok(Json(json!({
                "error": e.to_string()
            })))
        }
    }
}

pub async fn get_server_status(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let servers = state.servers.read().unwrap();
    match servers.get(&id) {
        Some(server) => {
            Ok(Json(json!({
                "id": server.id,
                "name": server.name,
                "status": server.status,
                "last_seen": server.last_seen,
                "is_connected": false
            })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn get_connection_stats(
    State(state): State<std::sync::Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let servers = state.servers.read().unwrap();
    let mut active_connections = 0;
    let mut oldest_connection_age = 0u64;
    let mut youngest_connection_age = 0u64;
    let now = chrono::Utc::now().timestamp() as u64;
    
    for server in servers.values() {
        if let Some(last_seen) = server.last_seen {
            active_connections += 1;
            let age = now - last_seen.timestamp() as u64;
            if oldest_connection_age == 0 || age > oldest_connection_age {
                oldest_connection_age = age;
            }
            if youngest_connection_age == 0 || age < youngest_connection_age {
                youngest_connection_age = age;
            }
        }
    }
    
    Ok(Json(json!({
        "active_connections": active_connections,
        "oldest_connection_age_seconds": oldest_connection_age,
        "youngest_connection_age_seconds": youngest_connection_age
    })))
}

pub async fn get_config_info(
    State(state): State<std::sync::Arc<AppState>>
) -> Result<Json<Value>, StatusCode> {
    let config = state.server_config.read().unwrap();
    Ok(Json(json!({
        "ssh_config_path": config.ssh_config_path
    })))
}

pub async fn get_connection_pool_details(
    State(state): State<std::sync::Arc<AppState>>
) -> Result<Json<Value>, StatusCode> {
    let servers = state.servers.read().unwrap();
    let ssh_connections = state.ssh_connections.read().unwrap();
    let now = chrono::Utc::now().timestamp() as u64;
    
    let mut server_connections = Vec::new();
    let mut active_ssh_connections = 0;
    let mut total_ssh_connections = ssh_connections.len();
    
    for server in servers.values() {
        let status = match &server.status {
            crate::models::ServerStatus::Online => "Online",
            crate::models::ServerStatus::Offline => "Offline",
            crate::models::ServerStatus::Connecting => "Connecting",
            crate::models::ServerStatus::Error(_) => "Error",
        };
        
        let last_seen_age = server.last_seen.map(|ls| now - ls.timestamp() as u64).unwrap_or(0);
        let next_monitoring_age = if server.next_monitoring > now {
            server.next_monitoring - now
        } else {
            0
        };
        
        server_connections.push(json!({
            "server_id": server.id,
            "server_name": server.name,
            "host": format!("{}:{}", server.host, server.port),
            "username": server.username,
            "status": status,
            "last_seen_age_seconds": last_seen_age,
            "next_monitoring_age_seconds": next_monitoring_age,
            "monitoring_interval_seconds": server.monitoring_interval.as_secs(),
            "has_ssh_connection": ssh_connections.values().any(|conn| conn.server_id == server.id && conn.is_active)
        }));
    }
    
    // Count active SSH connections
    for conn in ssh_connections.values() {
        if conn.is_active {
            active_ssh_connections += 1;
        }
    }
    
    Ok(Json(json!({
        "server_connections": server_connections,
        "ssh_connection_pool": {
            "active_connections": active_ssh_connections,
            "total_connections": total_ssh_connections,
            "connections": ssh_connections.values().map(|conn| json!({
                "server_id": conn.server_id,
                "connection_id": conn.connection_id,
                "is_active": conn.is_active,
                "last_used_age_seconds": now - conn.last_used
            })).collect::<Vec<_>>()
        },
        "summary": {
            "total_servers": servers.len(),
            "online_servers": servers.values().filter(|s| matches!(s.status, crate::models::ServerStatus::Online)).count(),
            "offline_servers": servers.values().filter(|s| matches!(s.status, crate::models::ServerStatus::Offline)).count(),
            "error_servers": servers.values().filter(|s| matches!(s.status, crate::models::ServerStatus::Error(_))).count(),
            "connecting_servers": servers.values().filter(|s| matches!(s.status, crate::models::ServerStatus::Connecting)).count()
        }
    })))
}

pub async fn get_server_details(
    State(state): State<std::sync::Arc<AppState>>,
    Path((id, metric)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    // Get latest monitoring data for the server
    let monitoring_data = state.get_latest_monitoring_data(&id);
    match monitoring_data {
        Some(data) => {
            let mut details = match metric.as_str() {
                "cpu" => json!(data.cpu),
                "memory" => json!(data.memory),
                "disks" => json!(data.disks),
                "network" => json!(data.network),
                "ports" => json!(data.ports),
                "ping" => json!(data.ping_tests),
                "system" => json!(data.system_info),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            // If data is empty or obviously invalid, add error field
            let invalid = match metric.as_str() {
                "cpu" => data.cpu.cores == 0,
                "memory" => data.memory.total == 0,
                "disks" => data.disks.is_empty(),
                "network" => data.network.is_empty(),
                "ports" => data.ports.is_empty(),
                "ping" => data.ping_tests.is_empty(),
                "system" => data.system_info.hostname.is_empty(),
                _ => false,
            };
            if invalid {
                details["error"] = serde_json::json!("No valid data available for metric");
            }
            Ok(Json(details))
        }
        None => Ok(Json(json!({ "error": "No monitoring data found for server" }))),
    }
}

#[derive(serde::Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: crate::models::AuthMethod,
    pub proxy_config: Option<crate::models::ProxyConfig>,
}

pub async fn get_server_history(
    State(state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    
    let historical_data = state.get_historical_data(&id, limit);
    Ok(Json(json!(historical_data)))
}

pub async fn start_monitoring(
    State(_state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // This would start a background monitoring task
    // For now, we'll just return success
    Ok(Json(json!({
        "message": "Monitoring started",
        "server_id": id
    })))
}

pub async fn stop_monitoring(
    State(_state): State<std::sync::Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // This would stop the background monitoring task
    // For now, we'll just return success
    Ok(Json(json!({
        "message": "Monitoring stopped",
        "server_id": id
    })))
}

#[derive(serde::Deserialize)]
pub struct UpdateServerRequest {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: crate::models::AuthMethod,
    pub proxy_config: Option<crate::models::ProxyConfig>,
}
