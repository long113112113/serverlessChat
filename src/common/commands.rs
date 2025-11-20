/// Lệnh UI gửi xuống tầng mạng.
#[derive(Debug, Clone)]
pub enum NetworkCommand {
    SendMessage(String),
}
