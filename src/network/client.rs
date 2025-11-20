use std::error::Error;

use chrono::Utc;
use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::swarm::{Config as SwarmConfig, SwarmEvent};
use libp2p::{PeerId, Swarm, identity, mdns};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::common::{ChatMessage, NetworkCommand, NetworkEvent};

use super::behavior::{ChatBehaviorEvent, build_behavior};
use super::transport::build_transport;

pub struct P2PClient {
    event_sender: mpsc::Sender<NetworkEvent>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
}

impl P2PClient {
    pub fn new(
        event_sender: mpsc::Sender<NetworkEvent>,
        command_receiver: mpsc::Receiver<NetworkCommand>,
    ) -> Self {
        Self {
            event_sender,
            command_receiver,
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("Local PeerID: {local_peer_id:?}");

        let transport = build_transport(&local_key)?;
        let (behavior, topic) = build_behavior(&local_key, local_peer_id)?;

        let mut swarm = Swarm::new(
            transport,
            behavior,
            local_peer_id,
            SwarmConfig::with_tokio_executor(),
        );

        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        log::info!("Network event loop started");

        loop {
            tokio::select! {
                command = self.command_receiver.recv() => {
                    if let Some(command) = command {
                        self.handle_command(command, &mut swarm, &topic, local_peer_id).await;
                    } else {
                        break;
                    }
                }
                event = swarm.select_next_some() => {
                    self.handle_swarm_event(event, &mut swarm).await;
                }
            }
        }

        Ok(())
    }

    async fn handle_command(
        &mut self,
        command: NetworkCommand,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
        topic: &gossipsub::IdentTopic,
        local_peer_id: PeerId,
    ) {
        match command {
            NetworkCommand::SendMessage(content) => {
                let msg = ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    sender: local_peer_id.to_string(),
                    content: content.clone(),
                    timestamp: Utc::now().timestamp(),
                };

                match serde_json::to_vec(&msg) {
                    Ok(json_bytes) => {
                        if let Err(err) = swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topic.clone(), json_bytes)
                        {
                            log::warn!("Publish error: {err:?}");
                        } else if let Err(err) = self
                            .event_sender
                            .send(NetworkEvent::MessageReceived(msg))
                            .await
                        {
                            log::warn!("Failed to notify UI about self message: {err:?}");
                        }
                    }
                    Err(err) => {
                        log::warn!("Failed to serialize message: {err:?}");
                    }
                }
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<ChatBehaviorEvent>,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
    ) {
        match event {
            SwarmEvent::Behaviour(ChatBehaviorEvent::Gossipsub(gossipsub::Event::Message {
                message,
                ..
            })) => {
                if let Ok(chat_msg) = serde_json::from_slice::<ChatMessage>(&message.data) {
                    let _ = self
                        .event_sender
                        .send(NetworkEvent::MessageReceived(chat_msg))
                        .await;
                }
            }
            SwarmEvent::Behaviour(ChatBehaviorEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _) in list {
                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    let _ = self
                        .event_sender
                        .send(NetworkEvent::PeerConnected(peer_id.to_string()))
                        .await;
                }
            }
            SwarmEvent::Behaviour(ChatBehaviorEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, _) in list {
                    swarm
                        .behaviour_mut()
                        .gossipsub
                        .remove_explicit_peer(&peer_id);
                    let _ = self
                        .event_sender
                        .send(NetworkEvent::PeerDisconnected(peer_id.to_string()))
                        .await;
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {address:?}");
            }
            _ => {}
        }
    }
}
