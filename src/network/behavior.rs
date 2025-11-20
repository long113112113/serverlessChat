use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use libp2p::gossipsub::{self, IdentTopic};
use libp2p::mdns;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, identity};

#[derive(NetworkBehaviour)]
pub struct ChatBehavior {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

pub fn build_behavior(
    local_key: &identity::Keypair,
    local_peer_id: PeerId,
) -> Result<(ChatBehavior, IdentTopic), Box<dyn Error>> {
    let message_id_fn = |message: &gossipsub::Message| {
        let mut hasher = DefaultHasher::new();
        message.data.hash(&mut hasher);
        gossipsub::MessageId::from(hasher.finish().to_string())
    };

    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()?;

    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )?;

    let topic = gossipsub::IdentTopic::new("rust-p2p-chat-global");
    gossipsub.subscribe(&topic)?;

    let mdns_behaviour = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

    Ok((
        ChatBehavior {
            gossipsub,
            mdns: mdns_behaviour,
        },
        topic,
    ))
}
