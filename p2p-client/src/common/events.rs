use super::types::{ChatMessage, PeerStatus};

/// Sự kiện từ tầng mạng gửi lên UI.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    MessageReceived(ChatMessage),
    HistorySynced(Vec<ChatMessage>),
    PeerConnected(String),
    PeerDisconnected(String),
    FriendStatus(PeerStatus),
}
