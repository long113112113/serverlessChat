use std::error::Error;

use chrono::Utc;
use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::identify;
use libp2p::kad;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{Config as SwarmConfig, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Swarm, identity};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::common::{ChatMessage, NetworkCommand, NetworkEvent};
use crate::config;

use super::behavior::{ChatBehaviorEvent, build_behavior};
use super::transport::build_transport;

pub struct P2PClient {
    event_sender: mpsc::Sender<NetworkEvent>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    bootstrap_peers: Vec<(PeerId, Multiaddr)>,
    enable_chat: bool,
    config_path: Option<String>,
    local_peer_id: Option<PeerId>,
}

impl P2PClient {
    pub fn new(
        event_sender: mpsc::Sender<NetworkEvent>,
        command_receiver: mpsc::Receiver<NetworkCommand>,
        bootstrap_peers: Vec<(PeerId, Multiaddr)>,
        enable_chat: bool,
        config_path: Option<String>,
    ) -> Self {
        Self {
            event_sender,
            command_receiver,
            bootstrap_peers,
            enable_chat,
            config_path,
            local_peer_id: None,
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        self.local_peer_id = Some(local_peer_id.clone());
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

        let bootstrap_peers = self.bootstrap_peers.clone();
        if bootstrap_peers.is_empty() {
            log::warn!("No bootstrap peers configured; update config JSON to enable WAN discovery");
        } else {
            for (peer_id, addr) in bootstrap_peers {
                log::info!("Adding bootstrap peer {peer_id} at {addr}");
                swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());
                if let Err(err) = swarm.dial(addr.clone()) {
                    log::warn!("Failed to dial bootstrap peer {peer_id}: {err}");
                }
            }

            if let Err(err) = swarm.behaviour_mut().kad.bootstrap() {
                log::warn!("Failed to trigger Kademlia bootstrap: {err}");
            }
        }

        log::info!("Network event loop started");

        loop {
            tokio::select! {
                command = self.command_receiver.recv(), if self.enable_chat => {
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
        if !self.enable_chat {
            return;
        }

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
            NetworkCommand::SyncRequest {
                to_peer,
                last_timestamp,
            } => {
                log::warn!(
                    "SyncRequest not implemented (to_peer={to_peer}, last_timestamp={last_timestamp})"
                );
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
            SwarmEvent::Behaviour(ChatBehaviorEvent::Identify(event)) => {
                self.handle_identify_event(event, swarm).await;
            }
            SwarmEvent::Behaviour(ChatBehaviorEvent::Kad(event)) => {
                self.handle_kad_event(event);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {address:?}");
                self.persist_self_address(&address).await;
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                let _ = self
                    .event_sender
                    .send(NetworkEvent::PeerConnected(peer_id.to_string()))
                    .await;
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let _ = self
                    .event_sender
                    .send(NetworkEvent::PeerDisconnected(peer_id.to_string()))
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_identify_event(
        &mut self,
        event: identify::Event,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
    ) {
        if let identify::Event::Received { peer_id, info, .. } = event {
            log::debug!(
                "Identify info from {peer_id}: protocols={:?}",
                info.protocols
            );

            // In server mode, persist peer addresses to bootstrap file
            if !self.enable_chat {
                if let Some(config_path) = &self.config_path {
                    for addr in &info.listen_addrs {
                        let full_addr = addr.clone().with(Protocol::P2p(peer_id));
                        config::add_peer_to_bootstrap_async(config_path, &full_addr.to_string())
                            .await;
                    }
                }
            }

            for addr in info.listen_addrs {
                swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());
            }
        }
    }

    fn handle_kad_event(&mut self, event: kad::Event) {
        match event {
            kad::Event::OutboundQueryProgressed { result, .. } => {
                if let kad::QueryResult::Bootstrap(res) = result {
                    match res {
                        Ok(kad::BootstrapOk { num_remaining, .. }) => {
                            log::info!("Kademlia bootstrap ok, remaining peers: {num_remaining}");
                        }
                        Err(err) => {
                            log::warn!("Kademlia bootstrap error: {err:?}");
                        }
                    }
                }
            }
            kad::Event::RoutingUpdated {
                peer, addresses, ..
            } => {
                log::debug!("Kademlia routing table updated for {peer} (addresses: {addresses:?})");
            }
            _ => {}
        }
    }

    async fn persist_self_address(&self, address: &Multiaddr) {
        if self.enable_chat {
            return;
        }

        let (Some(config_path), Some(peer_id)) =
            (self.config_path.as_ref(), self.local_peer_id.clone())
        else {
            return;
        };

        let full_addr = address.clone().with(Protocol::P2p(peer_id));
        config::persist_bootstrap_node_async(config_path, &full_addr.to_string()).await;
    }
}
