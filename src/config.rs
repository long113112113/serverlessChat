use regex;

use crate::storage::{ServerDatabase, ensure_data_dir};

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

pub async fn persist_bootstrap_node_async(_path: &str, entry: &str) {
    ensure_data_dir().ok();

    // Fetch public IP asynchronously
    let public_ip = fetch_public_ip().await.ok();

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

    // Use SQLite for server mode
    // First validate that the address is a valid multiaddr
    let addr = match entry_with_public.parse::<libp2p::Multiaddr>() {
        Ok(addr) => addr,
        Err(err) => {
            log::warn!(
                "Invalid multiaddr '{}', not persisting to database: {}",
                entry_with_public,
                err
            );
            return;
        }
    };

    // Extract peer_id and validate it exists
    let peer_id = match extract_peer_id(&addr) {
        Some(peer_id) => peer_id,
        None => {
            log::warn!(
                "Multiaddr '{}' missing /p2p/PeerId suffix, not persisting to database",
                entry_with_public
            );
            return;
        }
    };

    if let Ok(db) = ServerDatabase::new() {
        // Remove duplicates with same peer_id
        if let Err(err) = db.remove_duplicate_peer_id(&peer_id.to_string(), &entry_with_public) {
            log::warn!("Failed to remove duplicate peer_id: {}", err);
        }

        if let Err(err) = db.upsert_bootstrap_node(&entry_with_public, Some(&peer_id.to_string())) {
            log::error!("Failed to persist bootstrap node to SQLite: {}", err);
        } else {
            log::info!("Persisted bootstrap node {} to SQLite", entry_with_public);
        }
    } else {
        log::error!("Failed to open server database");
    }
}

/// Add a peer's address to bootstrap list (for server mode to track connected peers)
pub async fn add_peer_to_bootstrap_async(_path: &str, peer_addr: &str) {
    ensure_data_dir().ok();

    // First validate that the address is a valid multiaddr
    let addr = match peer_addr.parse::<libp2p::Multiaddr>() {
        Ok(addr) => addr,
        Err(err) => {
            log::warn!(
                "Invalid multiaddr '{}', not persisting to database: {}",
                peer_addr,
                err
            );
            return;
        }
    };

    // Extract peer_id and validate it exists
    let peer_id = match extract_peer_id(&addr) {
        Some(peer_id) => peer_id,
        None => {
            log::warn!(
                "Multiaddr '{}' missing /p2p/PeerId suffix, not persisting to database",
                peer_addr
            );
            return;
        }
    };

    if let Ok(db) = ServerDatabase::new() {
        // Remove existing entries with same peer ID
        if let Err(err) = db.remove_duplicate_peer_id(&peer_id.to_string(), peer_addr) {
            log::warn!("Failed to remove duplicate peer_id: {}", err);
        }

        if let Err(err) = db.upsert_bootstrap_node(peer_addr, Some(&peer_id.to_string())) {
            log::error!("Failed to add peer to bootstrap SQLite: {}", err);
        } else {
            log::debug!("Added peer {} to bootstrap SQLite", peer_addr);
        }
    } else {
        log::error!("Failed to open server database");
    }
}

/// Load bootstrap nodes from SQLite
pub fn load_bootstrap_nodes_from_db() -> Vec<String> {
    ensure_data_dir().ok();

    match ServerDatabase::new() {
        Ok(db) => match db.get_all_bootstrap_nodes() {
            Ok(nodes) => nodes.into_iter().map(|n| n.address).collect(),
            Err(err) => {
                log::warn!("Failed to load bootstrap nodes from SQLite: {}", err);
                Vec::new()
            }
        },
        Err(err) => {
            log::warn!("Failed to open server database: {}", err);
            Vec::new()
        }
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
