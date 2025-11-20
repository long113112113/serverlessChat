use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const DEFAULT_CONFIG_PATH: &str = "config/bootstrap.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: Vec::new(),
        }
    }
}

pub fn load_config(path: &str) -> AppConfig {
    let path = Path::new(path);
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("Failed to parse config file {}: {err}", path.display());
                AppConfig::default()
            }
        },
        Err(err) => {
            log::info!(
                "Config file {} not found ({err}); using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

pub fn save_config(path: &str, config: &AppConfig) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)
}

pub fn persist_bootstrap_node(path: &str, entry: &str) {
    let mut config = load_config(path);
    config.bootstrap_nodes.retain(|node| node != entry);
    config.bootstrap_nodes.insert(0, entry.to_string());

    if let Err(err) = save_config(path, &config) {
        log::error!("Failed to write bootstrap config {}: {err}", path);
    } else {
        log::info!("Persisted bootstrap node {} to {}", entry, path);
    }
}
