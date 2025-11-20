mod common;
mod config;
mod network;
mod ui;

use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use network::P2PClient;
use tokio::sync::mpsc;
use ui::ChatApp;

#[derive(Parser)]
#[command(
    name = "rust_p2p_chat",
    version,
    about = "Modular P2P chat application"
)]
struct Cli {
    /// Path to JSON config file
    #[arg(long, default_value = config::DEFAULT_CONFIG_PATH, value_name = "FILE")]
    config: String,
    #[command(subcommand)]
    mode: Option<Mode>,
}

#[derive(Subcommand, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Run in bootstrap/server mode (no UI, announce only)
    Server,
}

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    dotenv().ok();
    // Khởi tạo Logger để debug
    env_logger::init();

    let cli = Cli::parse();
    let app_config = config::load_config(&cli.config);
    let bootstrap_peers = parse_bootstrap_peers(&app_config.bootstrap_nodes);

    if cli.mode == Some(Mode::Server) {
        run_server_node(bootstrap_peers, cli.config.clone()).await;
        return Ok(());
    }

    run_full_client(bootstrap_peers).await
}

async fn run_server_node(bootstrap_peers: Vec<(PeerId, Multiaddr)>, config_path: String) {
    let (_cmd_tx, cmd_rx) = mpsc::channel(1);
    let (event_tx, _event_rx) = mpsc::channel(1);

    let client = P2PClient::new(event_tx, cmd_rx, bootstrap_peers, false, Some(config_path));
    if let Err(err) = client.run().await {
        log::error!("Bootstrap node terminated unexpectedly: {err}");
    }
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
        let client = P2PClient::new(event_tx, cmd_rx, bootstrap_clone, true, None);
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
