mod common;
mod network;
mod ui;

use network::P2PClient;
use tokio::sync::mpsc;
use ui::ChatApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Khởi tạo Logger để debug
    env_logger::init();

    // 1. Tạo các kênh giao tiếp (Channels)
    // UI -> Network
    let (cmd_tx, cmd_rx) = mpsc::channel(100);
    // Network -> UI
    let (event_tx, event_rx) = mpsc::channel(100);

    // 2. Khởi chạy Network Thread (Chạy ngầm)
    tokio::spawn(async move {
        let client = P2PClient::new(event_tx, cmd_rx);
        if let Err(err) = client.run().await {
            log::error!("Network client terminated: {err}");
        }
    });

    // 3. Khởi chạy UI (Chạy trên Main Thread)
    let options = eframe::NativeOptions::default();
    let mut event_rx = Some(event_rx);

    eframe::run_native(
        "Rust P2P Chat",
        options,
        Box::new(move |cc| {
            let event_receiver = event_rx
                .take()
                .expect("ChatApp should only be initialized once");

            Ok(Box::new(ChatApp::new(cc, cmd_tx.clone(), event_receiver)))
        }),
    )
}
