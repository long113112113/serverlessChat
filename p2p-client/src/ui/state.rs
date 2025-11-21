use crate::common::{ChatMessage, PeerStatus};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

/// Debug event để hiển thị thông tin mạng
#[derive(Debug, Clone)]
pub struct DebugEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub peer_id: Option<String>,
    pub message: String,
}

/// Trạng thái cục bộ của UI.
pub struct AppState {
    pub messages: Vec<ChatMessage>,
    pub input_text: String,
    pub peer_address_input: String,
    pub peers: Vec<String>,
    pub debug_events: Vec<DebugEvent>,
    /// Map peer_id -> last_seen timestamp để tính thời gian offline
    pub peer_last_seen: HashMap<String, DateTime<Utc>>,
    /// Input lưu peer_id bạn bè do người dùng nhập
    pub friend_input: String,
    /// Danh sách bạn bè (theo peer_id) và trạng thái mới nhất
    pub friends: BTreeMap<String, PeerStatus>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_text: String::new(),
            peer_address_input: String::new(),
            peers: Vec::new(),
            debug_events: Vec::new(),
            peer_last_seen: HashMap::new(),
            friend_input: String::new(),
            friends: BTreeMap::new(),
        }
    }

    pub fn push_message(&mut self, message: ChatMessage) {
        self.messages.push(message.clone());
        self.add_debug_event(
            "MESSAGE_RECEIVED".to_string(),
            Some(message.sender.clone()),
            format!(
                "Message from {}: {}",
                &message.sender[..8],
                &message.content
            ),
        );
    }

    pub fn push_history(&mut self, mut history: Vec<ChatMessage>) {
        self.messages.append(&mut history);
        self.messages.sort_by_key(|message| message.timestamp);
    }

    pub fn add_peer(&mut self, peer_id: String) {
        let now = Utc::now();
        let is_new = !self.peers.iter().any(|peer| peer == &peer_id);

        if is_new {
            self.peers.push(peer_id.clone());
        }

        // Cập nhật last_seen
        self.peer_last_seen.insert(peer_id.clone(), now);

        // Thêm debug event
        if is_new {
            self.add_debug_event(
                "PEER_CONNECTED".to_string(),
                Some(peer_id),
                format!("Peer connected at {}", now.format("%H:%M:%S")),
            );
        } else {
            // Peer đã tồn tại nhưng được refresh (có thể là mDNS refresh)
            self.add_debug_event(
                "PEER_REFRESHED".to_string(),
                Some(peer_id),
                format!("Peer refreshed at {}", now.format("%H:%M:%S")),
            );
        }
    }

    pub fn remove_peer(&mut self, peer_id: &str) {
        let now = Utc::now();
        let was_connected = self.peers.iter().any(|peer| peer == peer_id);

        if was_connected {
            self.peers.retain(|peer| peer != peer_id);

            // Tính thời gian đã online nếu có last_seen
            let duration_msg = if let Some(last_seen) = self.peer_last_seen.get(peer_id) {
                let duration = now.signed_duration_since(*last_seen);
                format!(
                    " (Was online for {:.1}s)",
                    duration.num_milliseconds() as f64 / 1000.0
                )
            } else {
                String::new()
            };

            self.add_debug_event(
                "PEER_DISCONNECTED".to_string(),
                Some(peer_id.to_string()),
                format!(
                    "Peer disconnected at {}{}",
                    now.format("%H:%M:%S"),
                    duration_msg
                ),
            );
        }
    }

    pub fn add_debug_event(
        &mut self,
        event_type: String,
        peer_id: Option<String>,
        message: String,
    ) {
        let event = DebugEvent {
            timestamp: Utc::now(),
            event_type,
            peer_id,
            message,
        };
        self.debug_events.push(event);

        // Giữ tối đa 100 events để không chiếm quá nhiều bộ nhớ
        if self.debug_events.len() > 100 {
            self.debug_events.remove(0);
        }
    }

    /// Tính thời gian không thấy peer (seconds)
    pub fn get_time_since_last_seen(&self, peer_id: &str) -> Option<f64> {
        if !self.peers.contains(&peer_id.to_string()) {
            // Peer đã disconnect, tính từ last_seen
            self.peer_last_seen.get(peer_id).map(|last_seen| {
                let now = Utc::now();
                let duration = now.signed_duration_since(*last_seen);
                duration.num_milliseconds() as f64 / 1000.0
            })
        } else {
            // Peer đang online, return None hoặc 0
            None
        }
    }

    pub fn upsert_friend_status(&mut self, status: PeerStatus) {
        if status.online {
            self.add_debug_event(
                "FRIEND_ONLINE".to_string(),
                Some(status.peer_id.clone()),
                format!("Friend online: {}", status.message),
            );
        } else {
            self.add_debug_event(
                "FRIEND_OFFLINE".to_string(),
                Some(status.peer_id.clone()),
                format!("Friend offline: {}", status.message),
            );
        }
        self.friends.insert(status.peer_id.clone(), status);
    }

    pub fn friend_statuses(&self) -> impl Iterator<Item = &PeerStatus> {
        self.friends.values()
    }
}
