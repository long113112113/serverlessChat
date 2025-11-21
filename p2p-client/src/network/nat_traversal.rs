use std::collections::{HashMap, HashSet};

use libp2p::autonat;
use libp2p::dcutr;
use libp2p::relay::client;
use libp2p::swarm::Swarm;
use libp2p::{Multiaddr, PeerId};

use super::behavior::ChatBehavior;

/// Handles NAT traversal mechanisms: Relay, DCUtR (hole punching), and AutoNAT
pub struct NatTraversal {
    /// Peers that failed direct connection attempts
    pub failed_direct_connections: HashSet<PeerId>,
    /// Known relay peers that can be used for relay connections
    pub relay_peers: HashSet<PeerId>,
    /// Pending relay retry attempts
    pub pending_relay_retries: HashMap<PeerId, Vec<Multiaddr>>,
    /// Bootstrap peers that might be relay servers
    bootstrap_peers: Vec<(PeerId, Multiaddr)>,
}

impl NatTraversal {
    pub fn new(bootstrap_peers: Vec<(PeerId, Multiaddr)>) -> Self {
        Self {
            failed_direct_connections: HashSet::new(),
            relay_peers: HashSet::new(),
            pending_relay_retries: HashMap::new(),
            bootstrap_peers,
        }
    }

    /// Handle relay events (reservation accepted, circuit established, etc.)
    pub async fn handle_relay_event(
        &mut self,
        event: client::Event,
        swarm: &mut Swarm<ChatBehavior>,
    ) {
        // Log event for debugging
        log::debug!("Relay client event: {:?}", event);
        
        // Handle reservation accepted - this is when we can listen on relay circuit
        // This is critical for receiving incoming connections through relay
        if let client::Event::ReservationReqAccepted { relay_peer_id, .. } = event {
            log::info!("Relay reservation request accepted from {}", relay_peer_id);
            // Track relay peer
            self.relay_peers.insert(relay_peer_id);
            
            // Listen on relay circuit address to receive incoming connections
            // Format: /p2p/<relay_peer_id>/p2p-circuit (modern libp2p uses /p2p/ instead of /ipfs/)
            // This allows other peers to connect to us through the relay
            let relay_circuit_addr = format!("/p2p/{}/p2p-circuit", relay_peer_id);
            match relay_circuit_addr.parse::<Multiaddr>() {
                Ok(addr) => {
                    match swarm.listen_on(addr.clone()) {
                        Ok(listener_id) => {
                            log::info!("Now listening on relay circuit: {} (listener_id: {:?})", addr, listener_id);
                            log::info!("Other peers can now connect to us via: {}", addr);
                        }
                        Err(err) => {
                            log::warn!("Failed to listen on relay circuit {}: {}", addr, err);
                        }
                    }
                }
                Err(err) => {
                    log::warn!("Failed to parse relay circuit address {}: {}", relay_circuit_addr, err);
                }
            }
        }
        // Note: Other event variants may have different names in client::Event
        // The exact structure depends on libp2p version
    }

    /// Handle AutoNAT events (NAT status detection)
    pub async fn handle_autonat_event(
        &mut self,
        event: autonat::Event,
        _swarm: &mut Swarm<ChatBehavior>,
    ) {
        match event {
            autonat::Event::StatusChanged { old: _, new: new_status } => {
                log::info!("Autonat status changed: {:?}", new_status);
                // Log NAT status for debugging
                if format!("{:?}", new_status).contains("Public") {
                    log::info!("Node is publicly reachable");
                } else if format!("{:?}", new_status).contains("Private") {
                    log::info!("Node is behind NAT, may need relay");
                } else {
                    log::debug!("NAT status: {:?}", new_status);
                }
            }
            _ => {
                log::debug!("Autonat event: {:?}", event);
            }
        }
    }

    /// Handle DCUtR events (Direct Connection Upgrade through Relay - hole punching)
    pub async fn handle_dcutr_event(
        &mut self,
        event: dcutr::Event,
        _swarm: &mut Swarm<ChatBehavior>,
    ) {
        // Log all DCUtR events for debugging and handle appropriately
        log::debug!("DCUtR event: {:?}", event);
        
        // Extract peer_id from event for processing
        // Note: The exact structure depends on libp2p version
        // This is a simplified handler that logs events
        // In practice, you would match on the specific event variants
        
        // Try to extract peer_id from event string representation for retry logic
        let event_str = format!("{:?}", event);
        if event_str.contains("Established") {
            // DCUtR succeeded - remove from failed connections
            // Note: In real implementation, extract peer_id from event
            log::info!("DCUtR hole punching established");
        } else if event_str.contains("Error") {
            // DCUtR failed - try relay as fallback
            log::warn!("DCUtR error occurred, will retry with relay if needed");
            // Note: In real implementation, extract peer_id and retry with relay
            // For now, we rely on the retry logic in OutgoingConnectionError handler
        } else {
            log::info!("DCUtR event: {}", event_str);
        }
    }

    /// Retry connection using relay when direct connection fails
    pub async fn retry_with_relay(
        &mut self,
        peer_id: PeerId,
        swarm: &mut Swarm<ChatBehavior>,
        dialed_peers: &HashSet<PeerId>,
    ) {
        // Skip if already connected or already retrying
        if dialed_peers.contains(&peer_id) || self.pending_relay_retries.contains_key(&peer_id) {
            return;
        }

        // Find a relay peer to use
        let relay_peer = self.relay_peers.iter().next().copied();
        
        if let Some(relay_id) = relay_peer {
            log::info!("Attempting to connect to {} via relay {}", peer_id, relay_id);
            
            // Construct relay address: /p2p/<relay_id>/p2p-circuit/p2p/<target_peer> (modern libp2p uses /p2p/)
            let relay_addr = format!("/p2p/{}/p2p-circuit/p2p/{}", relay_id, peer_id);
            
            match relay_addr.parse::<Multiaddr>() {
                Ok(addr) => {
                    // Store for retry tracking
                    self.pending_relay_retries.insert(peer_id, vec![addr.clone()]);
                    
                    match swarm.dial(addr.clone()) {
                        Ok(()) => {
                            log::info!("Dialing {} via relay {} initiated", peer_id, relay_id);
                        }
                        Err(err) => {
                            log::warn!("Failed to dial {} via relay {}: {}", peer_id, relay_id, err);
                            self.pending_relay_retries.remove(&peer_id);
                        }
                    }
                }
                Err(err) => {
                    log::warn!("Failed to parse relay address: {}", err);
                }
            }
        } else {
            log::debug!("No relay peer available for connecting to {}", peer_id);
            // Try to discover relay peers from bootstrap nodes
            self.discover_relay_peers(swarm).await;
        }
    }

    /// Discover relay peers from bootstrap nodes
    async fn discover_relay_peers(
        &mut self,
        _swarm: &mut Swarm<ChatBehavior>,
    ) {
        // Check connected peers for relay capability
        // In a real implementation, you might query DHT or check identify info
        for (peer_id, _) in &self.bootstrap_peers {
            // Bootstrap nodes might be relay servers
            if !self.relay_peers.contains(peer_id) {
                // Try to reserve a slot on bootstrap peer as relay
                // This is a simplified approach - in practice, you'd check if peer supports relay
                log::debug!("Checking if {} can be used as relay", peer_id);
                // Add bootstrap peers as potential relay servers
                // They will be confirmed when reservation is accepted
                self.relay_peers.insert(*peer_id);
            }
        }
    }

    /// Mark a peer as having failed direct connection
    pub fn mark_failed_direct(&mut self, peer_id: PeerId) {
        self.failed_direct_connections.insert(peer_id);
    }

    /// Remove a peer from failed connections (e.g., when connection succeeds)
    pub fn clear_failed_direct(&mut self, peer_id: &PeerId) {
        self.failed_direct_connections.remove(peer_id);
        self.pending_relay_retries.remove(peer_id);
    }

    /// Check if a peer has failed direct connection
    pub fn has_failed_direct(&self, peer_id: &PeerId) -> bool {
        self.failed_direct_connections.contains(peer_id)
    }
}

