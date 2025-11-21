use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use libp2p::gossipsub::{self, IdentTopic};
use libp2p::identify;
use libp2p::kad::{self, Mode as KadMode, store::MemoryStore};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, identity};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ChatBehaviorEvent")]
pub struct ChatBehavior {
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
}

#[allow(clippy::large_enum_variant)]
pub enum ChatBehaviorEvent {
    Gossipsub(gossipsub::Event),
    Kad(kad::Event),
    Identify(identify::Event),
}

impl From<gossipsub::Event> for ChatBehaviorEvent {
    fn from(event: gossipsub::Event) -> Self {
        ChatBehaviorEvent::Gossipsub(event)
    }
}

impl From<kad::Event> for ChatBehaviorEvent {
    fn from(event: kad::Event) -> Self {
        ChatBehaviorEvent::Kad(event)
    }
}

impl From<identify::Event> for ChatBehaviorEvent {
    fn from(event: identify::Event) -> Self {
        ChatBehaviorEvent::Identify(event)
    }
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
        .heartbeat_interval(Duration::from_secs(5))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()?;

    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )?;

    let topic = gossipsub::IdentTopic::new("rust-p2p-chat-global");
    gossipsub.subscribe(&topic)?;

    let store = MemoryStore::new(local_peer_id);
    let mut kad = kad::Behaviour::new(local_peer_id, store);
    kad.set_mode(Some(KadMode::Server));

    let identify_config =
        identify::Config::new("rust-p2p-chat/1.0.0".into(), local_key.public().clone());
    let identify = identify::Behaviour::new(identify_config);

    Ok((
        ChatBehavior {
            gossipsub,
            kad,
            identify,
        },
        topic,
    ))
}
