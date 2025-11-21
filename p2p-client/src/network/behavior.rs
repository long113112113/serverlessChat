use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use libp2p::autonat;
use libp2p::dcutr;
use libp2p::gossipsub::{self, IdentTopic};
use libp2p::identify;
use libp2p::kad::{self, Mode as KadMode, store::MemoryStore};
use libp2p::ping;
use libp2p::relay::client;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, identity};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ChatBehaviorEvent")]
pub struct ChatBehavior {
    pub gossipsub: gossipsub::Behaviour,
    pub kad: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub relay: client::Behaviour,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub ping: ping::Behaviour,
}

#[allow(clippy::large_enum_variant)]
pub enum ChatBehaviorEvent {
    Gossipsub(gossipsub::Event),
    Kad(kad::Event),
    Identify(identify::Event),
    Relay(client::Event),
    Autonat(autonat::Event),
    Dcutr(dcutr::Event),
    Ping(ping::Event),
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

impl From<client::Event> for ChatBehaviorEvent {
    fn from(event: client::Event) -> Self {
        ChatBehaviorEvent::Relay(event)
    }
}

impl From<autonat::Event> for ChatBehaviorEvent {
    fn from(event: autonat::Event) -> Self {
        ChatBehaviorEvent::Autonat(event)
    }
}

impl From<dcutr::Event> for ChatBehaviorEvent {
    fn from(event: dcutr::Event) -> Self {
        ChatBehaviorEvent::Dcutr(event)
    }
}

impl From<ping::Event> for ChatBehaviorEvent {
    fn from(event: ping::Event) -> Self {
        ChatBehaviorEvent::Ping(event)
    }
}

pub fn build_behavior(
    local_key: &identity::Keypair,
    local_peer_id: PeerId,
    relay_behaviour: libp2p::relay::client::Behaviour,
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

    // Relay behaviour is passed from transport.rs where it was created together with relay transport
    // This ensures Transport and Behaviour are properly linked
    let autonat = autonat::Behaviour::new(local_peer_id, autonat::Config::default());
    let dcutr = dcutr::Behaviour::new(local_peer_id);
    let ping = ping::Behaviour::new(ping::Config::default());

    Ok((
        ChatBehavior {
            gossipsub,
            kad,
            identify,
            relay: relay_behaviour,
            autonat,
            dcutr,
            ping,
        },
        topic,
    ))
}
