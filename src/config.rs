use std::fs;
use std::path::Path;

use regex;
use serde::{Deserialize, Serialize};

pub const DEFAULT_CONFIG_PATH: &str = "config/bootstrap.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,
    /// Optional public IP or domain to use instead of auto-detected private IP
    #[serde(default)]
    pub public_address: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: Vec::new(),
            public_address: None,
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

/// Replace private IP with public IP in multiaddr if available
pub fn replace_with_public_ip(multiaddr_str: &str, public_ip: Option<&str>) -> String {
    if let Some(public_ip) = public_ip {
        // Replace /ip4/10.x.x.x, /ip4/172.16-31.x.x, /ip4/192.168.x.x with public IP
        let private_ip_patterns = [
            r"/ip4/10\.\d+\.\d+\.\d+",
            r"/ip4/172\.(1[6-9]|2[0-9]|3[0-1])\.\d+\.\d+",
            r"/ip4/192\.168\.\d+\.\d+",
        ];

        let mut result = multiaddr_str.to_string();
        for pattern in &private_ip_patterns {
            let re = regex::Regex::new(pattern).unwrap();
            result = re
                .replace(&result, &format!("/ip4/{}", public_ip))
                .to_string();
        }
        result
    } else {
        multiaddr_str.to_string()
    }
}

pub async fn persist_bootstrap_node_async(path: &str, entry: &str) {
    let mut config = load_config(path);

    // Try to get public IP from config or fetch it
    let public_ip = if config.public_address.is_some() {
        config.public_address.clone()
    } else {
        // Try to fetch public IP from current runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(async { fetch_public_ip().await.ok() }),
            Err(_) => None,
        }
    };

    let entry_with_public = if let Some(public_ip) = &public_ip {
        let replaced = replace_with_public_ip(entry, Some(public_ip));
        log::info!(
            "Replaced private IP with public IP: {} -> {}",
            entry,
            replaced
        );
        replaced
    } else {
        entry.to_string()
    };

    // Remove all entries with same peer ID
    if let Ok(addr) = entry_with_public.parse::<libp2p::Multiaddr>() {
        if let Some(peer_id) = extract_peer_id(&addr) {
            config.bootstrap_nodes.retain(|node| {
                node.parse::<libp2p::Multiaddr>()
                    .ok()
                    .and_then(|a| extract_peer_id(&a))
                    .map(|pid| pid != peer_id)
                    .unwrap_or(true)
            });
        }
    }

    config.bootstrap_nodes.insert(0, entry_with_public.clone());

    if let Err(err) = save_config(path, &config) {
        log::error!("Failed to write bootstrap config {}: {err}", path);
    } else {
        log::info!("Persisted bootstrap node {} to {}", entry_with_public, path);
    }
}

fn extract_peer_id(addr: &libp2p::Multiaddr) -> Option<libp2p::PeerId> {
    use libp2p::multiaddr::Protocol;
    addr.iter().find_map(|p| match p {
        Protocol::P2p(peer_id) => Some(peer_id),
        _ => None,
    })
}

/// Fetch public IP from external service
pub async fn fetch_public_ip() -> Result<String, Box<dyn std::error::Error>> {
    // Try multiple services for reliability
    let services = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ];

    for service in &services {
        match reqwest::get(*service).await {
            Ok(resp) => {
                if let Ok(ip) = resp.text().await {
                    let ip = ip.trim().to_string();
                    if !ip.is_empty() && is_valid_ip(&ip) {
                        log::info!("Detected public IP: {}", ip);
                        return Ok(ip);
                    }
                }
            }
            Err(err) => {
                log::debug!("Failed to fetch from {}: {}", service, err);
            }
        }
    }

    Err("Unable to fetch public IP from any service".into())
}

fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>().is_ok()
}
