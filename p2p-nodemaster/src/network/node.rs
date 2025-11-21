use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::Path;

use libp2p::futures::StreamExt;
use libp2p::identify;
use libp2p::kad;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{Config as SwarmConfig, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, identity};
use tokio::time::{interval, Duration};

use super::behavior::{NodeBehaviorEvent, build_behavior};
use super::transport::build_transport;

const NODE_KEY_PATH: &str = "data/node_key.pk";

pub struct BootstrapNode {
    // In-memory storage of discovered peers and their addresses
    peers: HashMap<PeerId, HashSet<Multiaddr>>,
    local_peer_id: Option<PeerId>,
}

impl BootstrapNode {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            peers: HashMap::new(),
            local_peer_id: None,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let local_key = load_or_generate_key()?;
        let local_peer_id = PeerId::from(local_key.public());
        self.local_peer_id = Some(local_peer_id.clone());
        log::info!("Bootstrap Node PeerID: {local_peer_id:?}");

        let transport = build_transport(&local_key)?;
        let behavior = build_behavior(&local_key, local_peer_id.clone())?;

        let mut swarm = Swarm::new(
            transport,
            behavior,
            local_peer_id.clone(),
            SwarmConfig::with_tokio_executor(),
        );

        // Listen on all interfaces using fixed port 4001
        swarm.listen_on("/ip4/0.0.0.0/tcp/4001".parse()?)?;

        log::info!("Bootstrap node started on tcp/4001, waiting for connections...");

        let mut stats_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    self.handle_swarm_event(event, &mut swarm).await;
                }
                _ = stats_interval.tick() => {
                    log::info!("Statistics: {} known peers", self.known_peers_count());
                }
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<NodeBehaviorEvent>,
        swarm: &mut Swarm<super::behavior::NodeBehavior>,
    ) {
        match event {
            SwarmEvent::Behaviour(NodeBehaviorEvent::Identify(event)) => {
                self.handle_identify_event(event, swarm).await;
            }
            SwarmEvent::Behaviour(NodeBehaviorEvent::Kad(event)) => {
                self.handle_kad_event(event);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                if let Some(peer_id) = self.local_peer_id.clone() {
                    let full_addr = address.clone().with(Protocol::P2p(peer_id));
                    log::info!("Bootstrap node listening on: {}", full_addr);
                    log::info!("Clients can connect to: {}", full_addr);
                } else {
                    log::info!("Bootstrap node listening on: {}", address);
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                log::info!("Client connected: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                log::info!("Client disconnected: {}", peer_id);
            }
            _ => {}
        }
    }

    async fn handle_identify_event(
        &mut self,
        event: identify::Event,
        swarm: &mut Swarm<super::behavior::NodeBehavior>,
    ) {
        if let identify::Event::Received { peer_id, info, .. } = event {
            log::debug!(
                "Identify info from {peer_id}: protocols={:?}",
                info.protocols
            );

            // Add peer addresses to Kademlia DHT and in-memory map
            for addr in info.listen_addrs {
                swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());

                self.peers
                    .entry(peer_id)
                    .or_default()
                    .insert(addr.clone());

                // Try to print IP portion if present for clarity
                let ip_str = extract_ip(&addr).unwrap_or_else(|| addr.to_string());
                log::info!("Peer discovered: {} @ {}", peer_id, ip_str);
            }

            log::debug!("Known peers: {}", self.known_peers_count());
        }
    }

    fn handle_kad_event(&mut self, event: kad::Event) {
        match event {
            kad::Event::RoutingUpdated { peer, addresses, .. } => {
                // Merge addresses into in-memory map as well
                let entry = self.peers.entry(peer).or_default();
                for addr in addresses.iter() {
                    entry.insert(addr.clone());
                }
                log::debug!(
                    "Kademlia routing table updated for {} ({} addrs). Total peers: {}",
                    peer,
                    entry.len(),
                    self.known_peers_count()
                );
            }
            _ => {}
        }
    }

    pub fn known_peers_count(&self) -> usize {
        self.peers.len()
    }

    #[allow(dead_code)]
    pub fn known_peers(&self) -> &HashMap<PeerId, HashSet<Multiaddr>> {
        &self.peers
    }
}

fn extract_ip(addr: &Multiaddr) -> Option<String> {
    // Extract first IP4/IP6 component if present
    for p in addr.iter() {
        match p {
            Protocol::Ip4(ip) => return Some(ip.to_string()),
            Protocol::Ip6(ip) => return Some(ip.to_string()),
            _ => {}
        }
    }
    None
}

fn load_or_generate_key() -> Result<identity::Keypair, Box<dyn Error>> {
    let path = Path::new(NODE_KEY_PATH);
    if path.exists() {
        let bytes = fs::read(path)?;
        let keypair = identity::Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| format!("Failed to decode identity key: {}", e))?;
        log::info!("Loaded persisted identity key from {}", NODE_KEY_PATH);
        Ok(keypair)
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let keypair = identity::Keypair::generate_ed25519();
        let encoded = keypair
            .to_protobuf_encoding()
            .map_err(|e| format!("Failed to encode identity key: {}", e))?;
        fs::write(path, encoded)?;
        log::info!(
            "Generated new identity key and saved to {}",
            NODE_KEY_PATH
        );
        Ok(keypair)
    }
}
