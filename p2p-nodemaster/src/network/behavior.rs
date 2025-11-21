use std::error::Error;

use libp2p::autonat;
use libp2p::dcutr;
use libp2p::identify;
use libp2p::kad::{self, store::MemoryStore, Mode as KadMode};
use libp2p::ping;
use libp2p::relay;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{identity, PeerId};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NodeBehaviorEvent")]
pub struct NodeBehavior {
    pub kad: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub relay: relay::Behaviour,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub ping: ping::Behaviour,
}

#[allow(clippy::large_enum_variant)]
pub enum NodeBehaviorEvent {
    Kad(kad::Event),
    Identify(identify::Event),
    Relay(relay::Event),
    Autonat(autonat::Event),
    Dcutr(dcutr::Event),
    Ping(ping::Event),
}

impl From<kad::Event> for NodeBehaviorEvent {
    fn from(event: kad::Event) -> Self {
        NodeBehaviorEvent::Kad(event)
    }
}

impl From<identify::Event> for NodeBehaviorEvent {
    fn from(event: identify::Event) -> Self {
        NodeBehaviorEvent::Identify(event)
    }
}

impl From<relay::Event> for NodeBehaviorEvent {
    fn from(event: relay::Event) -> Self {
        NodeBehaviorEvent::Relay(event)
    }
}

impl From<autonat::Event> for NodeBehaviorEvent {
    fn from(event: autonat::Event) -> Self {
        NodeBehaviorEvent::Autonat(event)
    }
}

impl From<dcutr::Event> for NodeBehaviorEvent {
    fn from(event: dcutr::Event) -> Self {
        NodeBehaviorEvent::Dcutr(event)
    }
}

impl From<ping::Event> for NodeBehaviorEvent {
    fn from(event: ping::Event) -> Self {
        NodeBehaviorEvent::Ping(event)
    }
}

pub fn build_behavior(
    local_key: &identity::Keypair,
    local_peer_id: PeerId,
) -> Result<NodeBehavior, Box<dyn Error>> {
    // Configure Kademlia as server mode (bootstrap node)
    let store = MemoryStore::new(local_peer_id);
    let mut kad = kad::Behaviour::new(local_peer_id, store);
    kad.set_mode(Some(KadMode::Server));

    let identify_config =
        identify::Config::new("p2p-nodemaster/1.0.0".into(), local_key.public().clone());
    let identify = identify::Behaviour::new(identify_config);

    let relay_behaviour = relay::Behaviour::new(local_peer_id, relay::Config::default());
    let autonat = autonat::Behaviour::new(local_peer_id, autonat::Config::default());
    let dcutr = dcutr::Behaviour::new(local_peer_id);
    let ping = ping::Behaviour::new(ping::Config::default());

    Ok(NodeBehavior {
        kad,
        identify,
        relay: relay_behaviour,
        autonat,
        dcutr,
        ping,
    })
}
