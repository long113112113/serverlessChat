use serde::{Deserialize, Serialize};

/// Domain model đại diện một tin nhắn chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
}
