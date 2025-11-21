use serde::{Deserialize, Serialize};

/// Domain model đại diện một tin nhắn chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
}

/// Trạng thái của một peer trong danh sách bạn bè.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerStatus {
    pub peer_id: String,
    pub online: bool,
    pub message: String,
    pub checked_at: i64,
}
