use rusqlite::{Result as SqlResult, params};
use std::path::Path;

use super::database::Database;
use super::models::BootstrapNode;

/// Database for bootstrap server mode
pub struct ServerDatabase {
    db: Database,
}

impl ServerDatabase {
    /// Initialize server database at default location
    pub fn new() -> SqlResult<Self> {
        Self::with_path("data/server.db")
    }

    /// Initialize server database at custom path
    pub fn with_path<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let db = Database::new(path)?;
        let server_db = Self { db };
        server_db.init_schema()?;
        Ok(server_db)
    }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bootstrap_nodes (
                address TEXT PRIMARY KEY,
                peer_id TEXT,
                added_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                last_verified INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bootstrap_added_at ON bootstrap_nodes(added_at)",
            [],
        )?;

        Ok(())
    }

    /// Add or update a bootstrap node
    pub fn upsert_bootstrap_node(&self, address: &str, peer_id: Option<&str>) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "INSERT OR REPLACE INTO bootstrap_nodes (address, peer_id, added_at, last_verified)
             VALUES (?1, ?2, strftime('%s', 'now'), strftime('%s', 'now'))",
            params![address, peer_id],
        )?;
        Ok(())
    }

    /// Get all bootstrap nodes, ordered by added_at (newest first)
    pub fn get_all_bootstrap_nodes(&self) -> SqlResult<Vec<BootstrapNode>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(
            "SELECT address, peer_id, added_at, last_verified 
             FROM bootstrap_nodes 
             ORDER BY added_at DESC",
        )?;

        let nodes = stmt
            .query_map([], |row| {
                Ok(BootstrapNode {
                    address: row.get(0)?,
                    peer_id: row.get(1)?,
                    added_at: row.get(2)?,
                    last_verified: row.get(3)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(nodes)
    }

    /// Remove a bootstrap node by address
    pub fn remove_bootstrap_node(&self, address: &str) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "DELETE FROM bootstrap_nodes WHERE address = ?1",
            params![address],
        )?;
        Ok(())
    }

    /// Remove all nodes with same peer_id (except the one with matching address)
    pub fn remove_duplicate_peer_id(&self, peer_id: &str, keep_address: &str) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "DELETE FROM bootstrap_nodes 
             WHERE peer_id = ?1 AND address != ?2",
            params![peer_id, keep_address],
        )?;
        Ok(())
    }

    /// Get bootstrap node count
    pub fn count(&self) -> SqlResult<usize> {
        let conn = self.db.connection();
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM bootstrap_nodes", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}
