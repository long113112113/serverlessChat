use serde::{Deserialize, Serialize};

/// Bootstrap node entry (for server mode)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapNode {
    pub address: String,
    pub peer_id: Option<String>,
    pub added_at: i64,
    pub last_verified: Option<i64>,
}

/// Chat message (for client mode)
#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
    pub created_at: i64,
}

/// Known peer (for client mode)
#[derive(Debug, Clone)]
pub struct Peer {
    pub peer_id: String,
    pub last_seen: Option<i64>,
    pub first_seen: i64,
    pub address: Option<String>,
    pub is_bootstrap: bool,
}

/// Identity information (for both server and client)
#[derive(Debug, Clone)]
pub struct Identity {
    pub peer_id: String,
    pub keypair_encrypted: Option<Vec<u8>>,
    pub created_at: i64,
}
