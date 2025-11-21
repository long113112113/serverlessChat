use std::error::Error;

use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::{Boxed,OrTransport};
use libp2p::core::upgrade::Version;
use libp2p::relay::client;
use libp2p::{PeerId, Transport, identity, noise, tcp, yamux};

pub fn build_transport(
    local_key: &identity::Keypair,
    local_peer_id: PeerId,
) -> Result<(Boxed<(PeerId, StreamMuxerBox)>, client::Behaviour), Box<dyn Error>> {
    let (relay_transport, relay_behaviour) = client::new(local_peer_id);
    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
    let transport = OrTransport::new(tcp_transport, relay_transport)
        .upgrade(Version::V1)
        .authenticate(noise::Config::new(local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    Ok((transport, relay_behaviour))
}
