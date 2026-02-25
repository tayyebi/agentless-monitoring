use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server_port: u16,
    pub log_level: String,
    pub monitoring_interval: u64,
    pub ping_timeout: u64,
    pub ssh_timeout: u64,
    pub fallback_password: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_port: 8080,
            log_level: "info".to_string(),
            monitoring_interval: 30,
            ping_timeout: 5,
            ssh_timeout: 10,
            fallback_password: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = "config.json";

        if Path::new(config_path).exists() {
            let content = std::fs::read_to_string(config_path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            // Create config file if it doesn't exist
            let content = serde_json::to_string_pretty(&config)?;
            std::fs::write(config_path, content)?;
            Ok(config)
        }
    }
}
