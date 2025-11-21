use std::error::Error;

use libp2p::identify;
use libp2p::kad::{self, store::MemoryStore, Mode as KadMode};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{identity, PeerId};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NodeBehaviorEvent")]
pub struct NodeBehavior {
    pub kad: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
}

#[allow(clippy::large_enum_variant)]
pub enum NodeBehaviorEvent {
    Kad(kad::Event),
    Identify(identify::Event),
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

    Ok(NodeBehavior {
        kad,
        identify,
    })
}
