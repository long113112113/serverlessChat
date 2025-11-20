use crate::common::ChatMessage;

/// Trạng thái cục bộ của UI.
pub struct AppState {
    pub messages: Vec<ChatMessage>,
    pub input_text: String,
    pub peers: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input_text: String::new(),
            peers: Vec::new(),
        }
    }

    pub fn push_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    pub fn add_peer(&mut self, peer_id: String) {
        if !self.peers.iter().any(|peer| peer == &peer_id) {
            self.peers.push(peer_id);
        }
    }

    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.retain(|peer| peer != peer_id);
    }
}
