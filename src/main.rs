mod api;
mod cli;
mod config;
mod models;
mod monitoring;
mod ssh;

use anyhow::Result;
use axum::{
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, warn, error, Level};

use crate::cli::{Cli, Commands};
use clap::Parser;
use crate::config::AppConfig;
use crate::models::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { config } => {
            run_server(config).await
        }
    }
}

async fn run_server(config_path: std::path::PathBuf) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // Load configuration
    let config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content)?
    } else {
        AppConfig::load()?
    };
    let app_state = Arc::new(AppState::new(config).await?);
    
    // Load servers from SSH config
    if let Err(e) = app_state.load_servers_from_ssh_config().await {
        warn!("🔧 Failed to load servers from SSH config: {}", e);
    } else {
        info!("✅ Loaded servers from SSH config");
    }

    // Start monitoring loop
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::monitoring::MonitoringService::start_monitoring_loop(app_state_clone).await {
            error!("💥 Monitoring loop failed: {}", e);
        }
    });

    // Build application routes
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/servers", get(api::servers::list_servers))
        .route("/api/servers/:id", get(api::servers::get_server))
        .route("/api/servers/:id/connect", post(api::servers::connect_server))
        .route("/api/servers/:id/status", get(api::servers::get_server_status))
        .route("/api/servers/:id/details/:metric", get(api::servers::get_server_details))
        .route("/api/servers/:id/history", get(api::servers::get_server_history))
        .route("/api/servers/:id/start-monitoring", post(api::servers::start_monitoring))
        .route("/api/servers/:id/stop-monitoring", post(api::servers::stop_monitoring))
        .route("/api/jobs", get(api::servers::list_jobs))
        .route("/api/connection-stats", get(api::servers::get_connection_stats))
        .route("/api/connection-pool", get(api::servers::get_connection_pool_details))
        .route("/api/config-info", get(api::servers::get_config_info))
        .route("/api/health", get(health_check))
        .nest_service("/static", ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    info!("🚀 Server running on http://0.0.0.0:8080");
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../templates/index.html"))
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}