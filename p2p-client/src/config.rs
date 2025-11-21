use std::fs;

use serde_json;

use crate::storage::ensure_data_dir;

const BOOTSTRAP_FILE: &str = "data/bootstrap_nodes.json";
const PLACEHOLDER_ADDR: &str = "/ip4/YOUR-NODE-MASTER-IP/tcp/4001/p2p/NODE-MASTER-PEERID";

/// Load bootstrap nodes from JSON file
pub fn load_bootstrap_nodes() -> Vec<String> {
    ensure_data_dir().ok();

    match fs::read_to_string(BOOTSTRAP_FILE) {
        Ok(content) => match serde_json::from_str::<Vec<String>>(&content) {
            Ok(nodes) => nodes,
            Err(err) => {
                log::warn!(
                    "Failed to parse bootstrap_nodes.json ({}). Returning empty list.",
                    err
                );
                Vec::new()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_placeholder_file().unwrap_or_else(|e| {
                log::warn!("Unable to create {}: {}", BOOTSTRAP_FILE, e);
            });
            Vec::new()
        }
        Err(err) => {
            log::warn!(
                "Failed to read bootstrap_nodes.json ({}). Returning empty list.",
                err
            );
            Vec::new()
        }
    }
}

fn create_placeholder_file() -> std::io::Result<()> {
    let default = vec![PLACEHOLDER_ADDR.to_string()];
    let content = serde_json::to_string_pretty(&default).unwrap_or_else(|_| "[]".to_string());
    fs::write(BOOTSTRAP_FILE, content)
}
