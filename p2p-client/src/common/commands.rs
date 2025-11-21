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
    /// Connect to a peer manually by address
    /// - address: Multiaddr của peer (ví dụ: /ip4/192.168.1.1/tcp/9000/p2p/12D3KooW...)
    ConnectToPeer {
        address: String,
    },
    /// Add a peer by PeerId into the friend list and check their status.
    AddFriend {
        peer_id: String,
    },
}
