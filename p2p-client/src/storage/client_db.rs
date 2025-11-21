use rusqlite::{OptionalExtension, Result as SqlResult, params};
use std::path::Path;

use super::database::Database;
use super::models::{Identity, Message, Peer};

/// Database for client mode (messages, peers, identity)
pub struct ClientDatabase {
    db: Database,
}

impl ClientDatabase {
    /// Initialize client database at default location
    pub fn new() -> SqlResult<Self> {
        Self::with_path("data/client.db")
    }

    /// Initialize client database at custom path
    pub fn with_path<P: AsRef<Path>>(path: P) -> SqlResult<Self> {
        let db = Database::new(path)?;
        let client_db = Self { db };
        client_db.init_schema()?;
        Ok(client_db)
    }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.db.connection();

        // Messages table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;

        // Peers table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS peers (
                peer_id TEXT PRIMARY KEY,
                last_seen INTEGER,
                first_seen INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                address TEXT,
                is_bootstrap INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        // Identity table (single row)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                peer_id TEXT NOT NULL,
                keypair_encrypted BLOB,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;

        // Indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_peers_last_seen ON peers(last_seen)",
            [],
        )?;

        Ok(())
    }

    // ========== Messages ==========

    /// Insert a new message
    pub fn insert_message(&self, message: &Message) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "INSERT OR IGNORE INTO messages (id, sender, content, timestamp, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id,
                message.sender,
                message.content,
                message.timestamp,
                message.created_at
            ],
        )?;
        Ok(())
    }

    /// Get messages with pagination
    pub fn get_messages(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> SqlResult<Vec<Message>> {
        let conn = self.db.connection();
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT id, sender, content, timestamp, created_at 
             FROM messages 
             ORDER BY timestamp ASC 
             LIMIT ?1 OFFSET ?2",
        )?;

        let messages = stmt
            .query_map(params![limit, offset], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    sender: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(messages)
    }

    /// Get messages after a timestamp
    pub fn get_messages_after(&self, timestamp: i64) -> SqlResult<Vec<Message>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(
            "SELECT id, sender, content, timestamp, created_at 
             FROM messages 
             WHERE timestamp > ?1 
             ORDER BY timestamp ASC",
        )?;

        let messages = stmt
            .query_map(params![timestamp], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    sender: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(messages)
    }

    /// Get message count
    pub fn message_count(&self) -> SqlResult<usize> {
        let conn = self.db.connection();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    // ========== Peers ==========

    /// Upsert a peer
    pub fn upsert_peer(&self, peer: &Peer) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "INSERT OR REPLACE INTO peers (peer_id, last_seen, first_seen, address, is_bootstrap)
             VALUES (?1, ?2, COALESCE((SELECT first_seen FROM peers WHERE peer_id = ?1), strftime('%s', 'now')), ?3, ?4)",
            params![
                peer.peer_id,
                peer.last_seen,
                peer.address,
                if peer.is_bootstrap { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    /// Update peer last_seen
    pub fn update_peer_last_seen(&self, peer_id: &str, last_seen: i64) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "UPDATE peers SET last_seen = ?1 WHERE peer_id = ?2",
            params![last_seen, peer_id],
        )?;
        Ok(())
    }

    /// Get all peers
    pub fn get_all_peers(&self) -> SqlResult<Vec<Peer>> {
        let conn = self.db.connection();
        let mut stmt = conn.prepare(
            "SELECT peer_id, last_seen, first_seen, address, is_bootstrap 
             FROM peers 
             ORDER BY last_seen DESC",
        )?;

        let peers = stmt
            .query_map([], |row| {
                Ok(Peer {
                    peer_id: row.get(0)?,
                    last_seen: row.get(1)?,
                    first_seen: row.get(2)?,
                    address: row.get(3)?,
                    is_bootstrap: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(peers)
    }

    /// Remove a peer
    pub fn remove_peer(&self, peer_id: &str) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM peers WHERE peer_id = ?1", params![peer_id])?;
        Ok(())
    }

    // ========== Identity ==========

    /// Save identity (replace if exists)
    pub fn save_identity(&self, identity: &Identity) -> SqlResult<()> {
        let conn = self.db.connection();
        conn.execute(
            "INSERT OR REPLACE INTO identity (id, peer_id, keypair_encrypted, created_at)
             VALUES (1, ?1, ?2, COALESCE((SELECT created_at FROM identity WHERE id = 1), strftime('%s', 'now')))",
            params![identity.peer_id, identity.keypair_encrypted],
        )?;
        Ok(())
    }

    /// Get identity
    pub fn get_identity(&self) -> SqlResult<Option<Identity>> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT peer_id, keypair_encrypted, created_at FROM identity WHERE id = 1")?;

        let identity = stmt
            .query_row([], |row| {
                Ok(Identity {
                    peer_id: row.get(0)?,
                    keypair_encrypted: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })
            .optional()?;

        Ok(identity)
    }
}
