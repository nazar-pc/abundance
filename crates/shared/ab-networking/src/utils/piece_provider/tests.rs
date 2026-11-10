use crate::utils::multihash::ToMultihash;
use crate::utils::piece_provider::KnownClosestPeers;
use ab_core_primitives::pieces::PieceIndex;
use libp2p::kad::{K_VALUE, KBucketKey};
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use std::iter;
use std::net::{IpAddr, Ipv4Addr};

fn address(port: u16) -> Multiaddr {
    Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST)).with(Protocol::Tcp(port))
}

#[test]
fn known_closest_peers_keeps_closest() {
    let local_peer_id = PeerId::random();
    let key = KBucketKey::from(PieceIndex::from(42).to_multihash());
    let mut known_closest_peers = KnownClosestPeers::new(key);

    let mut peer_ids = iter::repeat_with(PeerId::random)
        .take(K_VALUE.get() * 3)
        .collect::<Vec<_>>();
    for &peer_id in &peer_ids {
        known_closest_peers.add_peer(&local_peer_id, peer_id, vec![address(1)]);
    }

    peer_ids.sort_by_key(|peer_id| KBucketKey::from(*peer_id).distance(&key));
    assert_eq!(
        known_closest_peers
            .iter()
            .map(|(peer_id, _addresses)| *peer_id)
            .collect::<Vec<_>>(),
        peer_ids[..K_VALUE.get()]
    );
}

#[test]
fn known_closest_peers_ignores_local_and_without_addresses() {
    let local_peer_id = PeerId::random();
    let key = KBucketKey::from(PieceIndex::from(42).to_multihash());
    let mut known_closest_peers = KnownClosestPeers::new(key);

    known_closest_peers.add_peer(&local_peer_id, local_peer_id, vec![address(1)]);
    known_closest_peers.add_peer(&local_peer_id, PeerId::random(), Vec::new());

    assert_eq!(known_closest_peers.iter().count(), 0);
}

#[test]
fn known_closest_peers_merges_addresses() {
    let local_peer_id = PeerId::random();
    let peer_id = PeerId::random();
    let key = KBucketKey::from(PieceIndex::from(42).to_multihash());
    let mut known_closest_peers = KnownClosestPeers::new(key);

    known_closest_peers.add_peer(&local_peer_id, peer_id, vec![address(1), address(2)]);
    known_closest_peers.add_peer(&local_peer_id, peer_id, vec![address(2), address(3)]);

    assert_eq!(
        known_closest_peers.iter().collect::<Vec<_>>(),
        [(&peer_id, &vec![address(1), address(2), address(3)])]
    );
}
