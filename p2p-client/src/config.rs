use crate::storage::{ServerDatabase, ensure_data_dir};

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
