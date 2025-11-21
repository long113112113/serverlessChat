use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::io;
use std::fs;
use std::path::Path;
use std::str::FromStr;

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

use crate::common::{ChatMessage, NetworkCommand, NetworkEvent, PeerStatus};
use serde_json;

use super::behavior::{ChatBehaviorEvent, build_behavior};
use super::transport::build_transport;

const CLIENT_KEY_PATH: &str = "data/client_key.pk";
const FRIENDS_FILE: &str = "data/friends.json";
const MAX_CONCURRENT_FRIEND_QUERIES: usize = 3;

pub struct P2PClient {
    event_sender: mpsc::Sender<NetworkEvent>,
    command_receiver: mpsc::Receiver<NetworkCommand>,
    bootstrap_peers: Vec<(PeerId, Multiaddr)>,
    enable_chat: bool,
    local_peer_id: Option<PeerId>,
    friend_ids: HashSet<String>,
    pending_friend_queries: HashMap<kad::QueryId, String>,
    friend_queue: VecDeque<String>,
}

impl P2PClient {
    pub fn new(
        event_sender: mpsc::Sender<NetworkEvent>,
        command_receiver: mpsc::Receiver<NetworkCommand>,
        bootstrap_peers: Vec<(PeerId, Multiaddr)>,
        enable_chat: bool,
    ) -> Self {
        let friend_ids = load_friend_list_from_disk();
        let friend_queue = friend_ids.iter().cloned().collect::<VecDeque<_>>();
        Self {
            event_sender,
            command_receiver,
            bootstrap_peers,
            enable_chat,
            local_peer_id: None,
            friend_ids,
            pending_friend_queries: HashMap::new(),
            friend_queue,
        }
    }

    async fn handle_add_friend(
        &mut self,
        peer_id: String,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
    ) {
        let peer_id = peer_id.trim().to_string();
        if peer_id.is_empty() {
            return;
        }

        let was_new = self.friend_ids.insert(peer_id.clone());
        if was_new {
            self.persist_friend_list();
        }

        match PeerId::from_str(&peer_id) {
            Ok(_) => {
                self.notify_friend_status(
                    &peer_id,
                    false,
                    "Đang kiểm tra qua bootstrap node...",
                )
                .await;
                self.enqueue_friend_check(&peer_id);
                self.try_start_next_friend_queries(swarm);
            }
            Err(err) => {
                self.notify_friend_status(
                    &peer_id,
                    false,
                    format!("PeerId không hợp lệ: {err}"),
                )
                .await;
            }
        }
    }

    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        let local_key = load_or_generate_local_key()?;
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
        self.emit_initial_friend_placeholders().await;
        self.enqueue_all_friend_checks();
        self.try_start_next_friend_queries(&mut swarm);

        loop {
            tokio::select! {
                command = self.command_receiver.recv() => {
                    match command {
                        Some(command) => {
                            self.handle_command(command, &mut swarm, &topic, local_peer_id).await;
                        }
                        None => break,
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
                if !self.enable_chat {
                    log::warn!("Chat feature disabled; ignoring SendMessage command");
                    return;
                }
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
                if !self.enable_chat {
                    log::warn!("Chat feature disabled; ignoring SyncRequest command");
                    return;
                }
                log::warn!(
                    "SyncRequest not implemented (to_peer={to_peer}, last_timestamp={last_timestamp})"
                );
            }
            NetworkCommand::ConnectToPeer { address } => {
                match address.parse::<Multiaddr>() {
                    Ok(addr) => {
                        log::info!("Attempting to connect to peer at {addr}");
                        
                        // Try to extract peer_id from multiaddr
                        let mut addr_clone = addr.clone();
                        if let Some(Protocol::P2p(peer_id)) = addr_clone.pop() {
                            // Add peer to Kademlia DHT
                            swarm.behaviour_mut().kad.add_address(&peer_id, addr.clone());
                        }
                        
                        // Attempt to dial the address
                        match swarm.dial(addr) {
                            Ok(()) => {
                                log::info!("Dial initiated successfully");
                                // Peer will be added when connection is established
                            }
                            Err(err) => {
                                log::error!("Failed to dial peer: {err}");
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Invalid multiaddr '{address}': {err}");
                    }
                }
            }
            NetworkCommand::AddFriend { peer_id } => {
                self.handle_add_friend(peer_id, swarm).await;
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
                self.handle_kad_event(event).await;
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {address:?}");
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                let peer_id_str = peer_id.to_string();
                let _ = self
                    .event_sender
                    .send(NetworkEvent::PeerConnected(peer_id_str.clone()))
                    .await;
                if self.friend_ids.contains(&peer_id_str) {
                    self.notify_friend_status(
                        &peer_id_str,
                        true,
                        "Đã kết nối trực tiếp tới bạn",
                    )
                    .await;
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let peer_id_str = peer_id.to_string();
                let _ = self
                    .event_sender
                    .send(NetworkEvent::PeerDisconnected(peer_id_str.clone()))
                    .await;
                if self.friend_ids.contains(&peer_id_str) {
                    self.notify_friend_status(
                        &peer_id_str,
                        false,
                        "Kết nối đã đóng",
                    )
                    .await;
                }
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


            for addr in info.listen_addrs {
                swarm
                    .behaviour_mut()
                    .kad
                    .add_address(&peer_id, addr.clone());
            }
        }
    }

    async fn handle_kad_event(&mut self, event: kad::Event) {
        match event {
            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                match result {
                    kad::QueryResult::Bootstrap(res) => {
                        match res {
                            Ok(kad::BootstrapOk { num_remaining, .. }) => {
                                log::info!(
                                    "Kademlia bootstrap ok, remaining peers: {num_remaining}"
                                );
                            }
                            Err(err) => {
                                log::warn!("Kademlia bootstrap error: {err:?}");
                            }
                        }
                    }
                    kad::QueryResult::GetClosestPeers(res) => {
                        if let Some(peer_id) = self.pending_friend_queries.remove(&id) {
                            self.handle_friend_lookup_result(peer_id, res).await;
                        }
                    }
                    _ => {}
                }
            }
            kad::Event::RoutingUpdated {
                peer, addresses, ..
            } => {
                log::debug!(
                    "Kademlia routing table updated for {peer} (addresses: {addresses:?})"
                );
            }
            _ => {}
        }
    }

    async fn notify_friend_status(
        &self,
        peer_id: &str,
        online: bool,
        message: impl Into<String>,
    ) {
        let status = PeerStatus {
            peer_id: peer_id.to_string(),
            online,
            message: message.into(),
            checked_at: Utc::now().timestamp(),
        };
        if let Err(err) = self
            .event_sender
            .send(NetworkEvent::FriendStatus(status))
            .await
        {
            log::warn!("Failed to emit friend status event: {err}");
        }
    }

    async fn emit_initial_friend_placeholders(&self) {
        for peer_id in &self.friend_ids {
            self.notify_friend_status(
                peer_id,
                false,
                "Đang chờ kiểm tra trạng thái qua bootstrap...",
            )
            .await;
        }
    }

    fn enqueue_all_friend_checks(&mut self) {
        let all_ids: Vec<String> = self.friend_ids.iter().cloned().collect();
        for peer_id in all_ids {
            self.enqueue_friend_check(&peer_id);
        }
    }

    fn enqueue_friend_check(&mut self, peer_id: &str) {
        if self
            .pending_friend_queries
            .values()
            .any(|running| running == peer_id)
        {
            return;
        }
        if self.friend_queue.iter().any(|queued| queued == peer_id) {
            return;
        }
        self.friend_queue.push_back(peer_id.to_string());
    }

    fn try_start_next_friend_queries(
        &mut self,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
    ) {
        while self.pending_friend_queries.len() < MAX_CONCURRENT_FRIEND_QUERIES {
            let Some(peer_id) = self.friend_queue.pop_front() else {
                break;
            };
            if !self.start_friend_lookup(&peer_id, swarm) {
                // Invalid peer id, continue to next
                continue;
            }
        }
    }

    fn start_friend_lookup(
        &mut self,
        peer_id: &str,
        swarm: &mut Swarm<super::behavior::ChatBehavior>,
    ) -> bool {
        match PeerId::from_str(peer_id) {
            Ok(target_peer) => {
                let query_id = swarm.behaviour_mut().kad.get_closest_peers(target_peer);
                self.pending_friend_queries
                    .insert(query_id, peer_id.to_string());
                true
            }
            Err(err) => {
                log::warn!("Không thể parse peer_id {peer_id}: {err}");
                false
            }
        }
    }

    async fn handle_friend_lookup_result(
        &self,
        peer_id: String,
        result: Result<kad::GetClosestPeersOk, kad::GetClosestPeersError>,
    ) {
        match result {
            Ok(kad::GetClosestPeersOk { peers, .. }) => {
                let target = PeerId::from_str(&peer_id).ok();
                let found = target
                    .as_ref()
                    .map(|target_peer| {
                        peers
                            .iter()
                            .any(|peer| peer.peer_id == *target_peer)
                    })
                    .unwrap_or(false);
                let message = if found {
                    format!("Đã tìm thấy trong DHT ({} peers gần nhất)", peers.len())
                } else if peers.is_empty() {
                    "Không tìm thấy peer trong DHT".to_string()
                } else {
                    format!(
                        "Không tìm thấy, nhưng có {} peers gần nhất trả về",
                        peers.len()
                    )
                };
                self.notify_friend_status(&peer_id, found, message).await;
            }
            Err(kad::GetClosestPeersError::Timeout { peers, .. }) => {
                let message = format!(
                    "Timeout khi truy vấn bootstrap ({} peers gần nhất)",
                    peers.len()
                );
                self.notify_friend_status(&peer_id, false, message).await;
            }
        }
    }

    fn persist_friend_list(&self) {
        if let Err(err) = write_friend_list(&self.friend_ids) {
            log::warn!("Failed to persist friend list: {err}");
        }
    }
}

fn load_or_generate_local_key() -> Result<identity::Keypair, Box<dyn Error>> {
    let path = Path::new(CLIENT_KEY_PATH);
    if path.exists() {
        let bytes = fs::read(path)?;
        let keypair = identity::Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| format!("Failed to decode client identity key: {}", e))?;
        log::info!("Loaded persisted client identity key from {}", CLIENT_KEY_PATH);
        Ok(keypair)
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let keypair = identity::Keypair::generate_ed25519();
        let encoded = keypair
            .to_protobuf_encoding()
            .map_err(|e| format!("Failed to encode client identity key: {}", e))?;
        fs::write(path, encoded)?;
        log::info!(
            "Generated new client identity key and saved to {}",
            CLIENT_KEY_PATH
        );
        Ok(keypair)
    }
}

fn load_friend_list_from_disk() -> HashSet<String> {
    match fs::read_to_string(FRIENDS_FILE) {
        Ok(content) => match serde_json::from_str::<Vec<String>>(&content) {
            Ok(list) => list.into_iter().collect(),
            Err(err) => {
                log::warn!("Failed to parse friends list: {err}");
                HashSet::new()
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => HashSet::new(),
        Err(err) => {
            log::warn!("Failed to read friends list: {err}");
            HashSet::new()
        }
    }
}

fn write_friend_list(friends: &HashSet<String>) -> io::Result<()> {
    let mut entries: Vec<String> = friends.iter().cloned().collect();
    entries.sort();

    if let Some(parent) = Path::new(FRIENDS_FILE).parent() {
        fs::create_dir_all(parent)?;
    }

    let payload = serde_json::to_string_pretty(&entries)?;
    fs::write(FRIENDS_FILE, payload)
}
