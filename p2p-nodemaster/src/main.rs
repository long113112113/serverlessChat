mod network;

use dotenvy::dotenv;
use network::node::BootstrapNode;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    env_logger::init();

    log::info!("Starting P2P Node Master (Bootstrap Node)...");

    let mut node = BootstrapNode::new()?;

    tokio::select! {
        result = node.run() => {
            if let Err(err) = result {
                log::error!("Bootstrap node error: {}", err);
            }
        }
        _ = signal::ctrl_c() => {
            log::info!("Received shutdown signal, stopping bootstrap node...");
        }
    }

    log::info!("Final statistics: {} known peers", node.known_peers_count());

    Ok(())
}
