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

#[cfg(not(miri))]
#[tokio::test]
async fn get_piece_from_cache() {
    use crate::protocols::request_response::handlers::cached_piece_by_index::{
        CachedPieceByIndexRequest, CachedPieceByIndexRequestHandler, CachedPieceByIndexResponse,
        PieceResult,
    };
    use crate::utils::piece_provider::{NoPieceValidator, PieceProvider};
    use crate::{Config, KademliaMode, construct};
    use ab_core_primitives::pieces::Piece;
    use async_lock::Semaphore;
    use futures::channel::oneshot;
    use libp2p::kad::Mode;
    use parking_lot::Mutex;
    use std::sync::Arc as StdArc;
    use std::time::Duration;
    use triomphe::Arc;

    let piece_index = PieceIndex::ZERO;
    let piece = Piece::default();

    let config_1 = Config {
        listen_on: vec!["/ip4/0.0.0.0/tcp/0".parse().unwrap()],
        allow_non_global_addresses_in_dht: true,
        kademlia_mode: KademliaMode::Static(Mode::Server),
        request_response_protocols: vec![CachedPieceByIndexRequestHandler::create({
            let piece = piece.clone();

            move |_, request: CachedPieceByIndexRequest| {
                let result =
                    (request.piece_index == piece_index).then(|| PieceResult::Piece(piece.clone()));

                async move {
                    Some(CachedPieceByIndexResponse {
                        result: result?,
                        cached_pieces: Vec::new(),
                    })
                }
            }
        })],
        ..Config::default()
    };
    let (node_1, mut node_runner_1) = construct(config_1).unwrap();

    let (node_1_address_sender, node_1_address_receiver) = oneshot::channel();
    let on_new_listener_handler = node_1.on_new_listener(StdArc::new({
        let node_1_address_sender = Mutex::new(Some(node_1_address_sender));

        move |address| {
            if matches!(address.iter().next(), Some(Protocol::Ip4(_)))
                && let Some(node_1_address_sender) = node_1_address_sender.lock().take()
            {
                node_1_address_sender.send(address.clone()).unwrap();
            }
        }
    }));

    tokio::spawn(async move {
        node_runner_1.run().await;
    });

    // Wait for first node to know its address
    let node_1_addr = node_1_address_receiver.await.unwrap();
    drop(on_new_listener_handler);

    let config_2 = Config {
        listen_on: vec!["/ip4/0.0.0.0/tcp/0".parse().unwrap()],
        allow_non_global_addresses_in_dht: true,
        bootstrap_addresses: vec![node_1_addr.with_p2p(node_1.id()).unwrap()],
        request_response_protocols: vec![CachedPieceByIndexRequestHandler::create(|_, _| async {
            None
        })],
        ..Config::default()
    };
    let (node_2, mut node_runner_2) = construct(config_2).unwrap();

    tokio::spawn(async move {
        node_runner_2.run().await;
    });

    // Wait for the first node to be identified as a server
    tokio::time::timeout(Duration::from_secs(10), async {
        while node_2.connected_servers().await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();

    let piece_provider = PieceProvider::new(node_2, NoPieceValidator, Arc::new(Semaphore::new(1)));

    assert_eq!(
        piece_provider.get_piece_from_cache(piece_index).await,
        Some(piece)
    );
}
