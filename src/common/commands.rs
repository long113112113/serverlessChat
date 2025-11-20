/// Lệnh UI gửi xuống tầng mạng.
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    SendMessage(String),
    /// Yêu cầu Peer đồng bộ tin nhắn (Offline-first logic)
    /// - to_peer: ID của người muốn đồng bộ
    /// - last_timestamp: Thời điểm cuối cùng mình nhận tin từ họ
    SyncRequest {
        to_peer: String,
        last_timestamp: i64,
    },
}
