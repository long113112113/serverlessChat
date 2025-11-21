mod common;
mod config;
mod network;
mod storage;
mod ui;

use dotenvy::dotenv;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use network::P2PClient;
use tokio::sync::mpsc;
use ui::ChatApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    dotenv().ok();
    // Khởi tạo Logger để debug
    env_logger::init();

    // Ensure data directory exists
    storage::ensure_data_dir().ok();

    // Load bootstrap nodes from SQLite
    let bootstrap_nodes = config::load_bootstrap_nodes_from_db();
    let bootstrap_peers = parse_bootstrap_peers(&bootstrap_nodes);

    run_full_client(bootstrap_peers).await
}

async fn run_full_client(bootstrap_peers: Vec<(PeerId, Multiaddr)>) -> Result<(), eframe::Error> {
    // 1. Tạo các kênh giao tiếp (Channels)
    // UI -> Network
    let (cmd_tx, cmd_rx) = mpsc::channel(100);
    // Network -> UI
    let (event_tx, event_rx) = mpsc::channel(100);

    // 2. Khởi chạy Network Thread (Chạy ngầm)
    let bootstrap_clone = bootstrap_peers.clone();
    tokio::spawn(async move {
        let client = P2PClient::new(event_tx, cmd_rx, bootstrap_clone, true);
        if let Err(err) = client.run().await {
            log::error!("Network client terminated: {err}");
        }
    });

    // 3. Khởi chạy UI (Chạy trên Main Thread)
    let options = eframe::NativeOptions::default();
    let mut event_rx = Some(event_rx);
    let bootstrap_peers = bootstrap_peers.clone();

    eframe::run_native(
        "Rust P2P Chat",
        options,
        Box::new(move |cc| {
            let event_receiver = event_rx
                .take()
                .expect("ChatApp should only be initialized once");

            log::info!(
                "Client started with {} bootstrap peers",
                bootstrap_peers.len()
            );

            Ok(Box::new(ChatApp::new(cc, cmd_tx.clone(), event_receiver)))
        }),
    )
}

fn parse_bootstrap_peers(entries: &[String]) -> Vec<(PeerId, Multiaddr)> {
    entries
        .iter()
        .filter_map(|entry| {
            let mut addr: Multiaddr = match entry.parse() {
                Ok(addr) => addr,
                Err(err) => {
                    log::warn!("Invalid multiaddr `{entry}`: {err}");
                    return None;
                }
            };

            let peer_id = match addr.pop() {
                Some(Protocol::P2p(peer)) => peer,
                _ => {
                    log::warn!("Multiaddr `{entry}` missing /p2p/PeerId suffix");
                    return None;
                }
            };

            Some((peer_id, addr))
        })
        .collect()
}
