use crate::common::ChatMessage;
use chrono::{DateTime, Utc};

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
    pub peers: Vec<String>,
    pub debug_events: Vec<DebugEvent>,
    /// Map peer_id -> last_seen timestamp để tính thời gian offline
    pub peer_last_seen: std::collections::HashMap<String, DateTime<Utc>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_text: String::new(),
            peers: Vec::new(),
            debug_events: Vec::new(),
            peer_last_seen: std::collections::HashMap::new(),
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
}
