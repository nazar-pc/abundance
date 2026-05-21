use ab_farmer::KNOWN_PEERS_CACHE_SIZE;
use ab_farmer::farm::plotted_pieces::PlottedPieces;
use ab_farmer::farmer_cache::FarmerCaches;
use ab_farmer::node_client::NodeClientExt;
use ab_farmer_rpc_primitives::MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST;
use ab_networking::libp2p::Multiaddr;
use ab_networking::libp2p::identity::Keypair;
use ab_networking::libp2p::multiaddr::Protocol;
use ab_networking::protocols::request_response::handlers::cached_piece_by_index::{
    CachedPieceByIndexRequest, CachedPieceByIndexRequestHandler, CachedPieceByIndexResponse,
    PieceResult,
};
use ab_networking::protocols::request_response::handlers::piece_by_index::{
    PieceByIndexRequest, PieceByIndexRequestHandler, PieceByIndexResponse,
};
use ab_networking::protocols::request_response::handlers::segment_header::{
    SuperSegmentHeaderBySegmentIndexesRequestHandler, SuperSegmentHeaderRequest,
    SuperSegmentHeaderResponse,
};
use ab_networking::utils::multihash::ToMultihash;
use ab_networking::utils::strip_peer_id;
use ab_networking::{
    Config, KademliaMode, KnownPeersManager, KnownPeersManagerConfig, Node, NodeRunner, WeakNode,
    construct,
};
use async_lock::RwLock as AsyncRwLock;
use clap::Parser;
use parking_lot::Mutex;
use prometheus_client::registry::Registry;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::sync::{Arc, Weak};
use tracing::{Instrument, debug, error, info, warn};

/// How many super segment headers can be requested at a time.
///
/// Must be the same as RPC limit since all requests go to the node anyway.
const SUPER_SEGMENT_HEADERS_LIMIT: u32 = MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST as u32;

/// Configuration for network stack
#[derive(Debug, Parser)]
pub(in super::super) struct NetworkArgs {
    /// Multiaddrs of bootstrap nodes to connect to on startup, multiple are supported
    #[arg(long = "bootstrap-node")]
    pub(in super::super) bootstrap_nodes: Vec<Multiaddr>,
    /// Multiaddrs to listen on for the networking, for instance, `/ip4/0.0.0.0/tcp/0`,
    /// multiple are supported.
    #[arg(long, default_values_t = [
        Multiaddr::from(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .with(Protocol::Tcp(30533)),
        Multiaddr::from(IpAddr::V6(Ipv6Addr::UNSPECIFIED))
            .with(Protocol::Tcp(30533))
    ])]
    pub(in super::super) listen_on: Vec<Multiaddr>,
    /// Enable non-global (private, shared, loopback..) addresses in Kademlia DHT.
    /// By default, these addresses are excluded from the DHT.
    #[arg(long, default_value_t = false)]
    pub(in super::super) allow_private_ips: bool,
    /// Multiaddrs of reserved nodes to maintain a connection to, multiple are supported
    #[arg(long)]
    pub(in super::super) reserved_peers: Vec<Multiaddr>,
    /// Maximum established incoming connection limit.
    #[arg(long, default_value_t = 300)]
    pub(in super::super) in_connections: u32,
    /// Maximum established outgoing swarm connection limit.
    #[arg(long, default_value_t = 100)]
    pub(in super::super) out_connections: u32,
    /// Maximum pending incoming connection limit.
    #[arg(long, default_value_t = 100)]
    pub(in super::super) pending_in_connections: u32,
    /// Maximum pending outgoing swarm connection limit.
    #[arg(long, default_value_t = 100)]
    pub(in super::super) pending_out_connections: u32,
    /// Known external addresses.
    #[arg(long = "external-address")]
    pub(in super::super) external_addresses: Vec<Multiaddr>,
}

#[expect(clippy::too_many_arguments)]
pub(in super::super) fn configure_network<FarmIndex, NC>(
    protocol_prefix: String,
    base_path: &Path,
    keypair: Keypair,
    NetworkArgs {
        listen_on,
        bootstrap_nodes,
        allow_private_ips,
        reserved_peers,
        in_connections,
        out_connections,
        pending_in_connections,
        pending_out_connections,
        external_addresses,
    }: NetworkArgs,
    weak_plotted_pieces: Weak<AsyncRwLock<PlottedPieces<FarmIndex>>>,
    node_client: NC,
    farmer_caches: FarmerCaches,
    prometheus_metrics_registry: Option<&mut Registry>,
) -> Result<(Node, NodeRunner), anyhow::Error>
where
    FarmIndex: Hash + Eq + Copy + fmt::Debug + Send + Sync + 'static,
    usize: From<FarmIndex>,
    NC: NodeClientExt + Clone,
{
    let known_peers_registry = KnownPeersManager::new(KnownPeersManagerConfig {
        path: Some(base_path.join("known_addresses.bin").into_boxed_path()),
        ignore_peer_list: strip_peer_id(bootstrap_nodes.clone())
            .into_iter()
            .map(|(peer_id, _)| peer_id)
            .collect::<HashSet<_>>(),
        cache_size: KNOWN_PEERS_CACHE_SIZE,
        ..Default::default()
    })
    .map(Box::new)?;

    let maybe_weak_node = Arc::new(Mutex::new(None::<WeakNode>));
    let default_config = Config::new(protocol_prefix, keypair, prometheus_metrics_registry);
    let config = Config {
        reserved_peers,
        listen_on,
        allow_non_global_addresses_in_dht: allow_private_ips,
        known_peers_registry,
        request_response_protocols: vec![
            {
                let maybe_weak_node = Arc::clone(&maybe_weak_node);
                let farmer_caches = farmer_caches.clone();

                CachedPieceByIndexRequestHandler::create(move |peer_id, request| {
                    let CachedPieceByIndexRequest {
                        piece_index,
                        cached_pieces,
                    } = request;
                    debug!(?piece_index, "Cached piece request received");

                    let maybe_weak_node = Arc::clone(&maybe_weak_node);
                    let farmer_caches = farmer_caches.clone();
                    let mut cached_pieces = Arc::unwrap_or_clone(cached_pieces);

                    async move {
                        let piece_from_cache =
                            farmer_caches.get_piece(piece_index.to_multihash()).await;
                        cached_pieces.truncate(CachedPieceByIndexRequest::RECOMMENDED_LIMIT);
                        let cached_pieces = farmer_caches.has_pieces(cached_pieces).await;

                        Some(CachedPieceByIndexResponse {
                            result: if let Some(piece) = piece_from_cache {
                                PieceResult::Piece(piece)
                            } else {
                                let maybe_node = maybe_weak_node
                                    .lock()
                                    .as_ref()
                                    .expect("Always called after network instantiation; qed")
                                    .upgrade();

                                let closest_peers = if let Some(node) = maybe_node {
                                    node.get_closest_local_peers(
                                        piece_index.to_multihash(),
                                        Some(peer_id),
                                    )
                                    .await
                                    .inspect_err(|error| {
                                        warn!(%error, "Failed to get closest local peers");
                                    })
                                    .unwrap_or_default()
                                } else {
                                    Vec::new()
                                };

                                PieceResult::ClosestPeers(closest_peers.into())
                            },
                            cached_pieces,
                        })
                    }
                    .in_current_span()
                })
            },
            PieceByIndexRequestHandler::create(move |_, request| {
                let PieceByIndexRequest {
                    piece_index,
                    cached_pieces,
                } = request;
                debug!(?piece_index, "Piece request received. Trying cache...");

                let weak_plotted_pieces = Weak::clone(&weak_plotted_pieces);
                let farmer_caches = farmer_caches.clone();
                let mut cached_pieces = Arc::unwrap_or_clone(cached_pieces);

                async move {
                    let piece_from_cache =
                        farmer_caches.get_piece(piece_index.to_multihash()).await;
                    cached_pieces.truncate(PieceByIndexRequest::RECOMMENDED_LIMIT);
                    let cached_pieces = farmer_caches.has_pieces(cached_pieces).await;

                    if let Some(piece) = piece_from_cache {
                        Some(PieceByIndexResponse {
                            piece: Some(piece),
                            cached_pieces,
                        })
                    } else {
                        debug!(
                            ?piece_index,
                            "No piece in the cache. Trying archival storage..."
                        );

                        let read_piece_fut =
                            if let Some(plotted_pieces) = weak_plotted_pieces.upgrade() {
                                plotted_pieces
                                    .try_read()?
                                    .read_piece(piece_index)?
                                    .in_current_span()
                            } else {
                                debug!("A readers and pieces are already dropped");
                                return None;
                            };

                        let piece = read_piece_fut.await;

                        Some(PieceByIndexResponse {
                            piece,
                            cached_pieces,
                        })
                    }
                }
                .in_current_span()
            }),
            SuperSegmentHeaderBySegmentIndexesRequestHandler::create(move |peer_id, req| {
                debug!(%peer_id, ?req, "Segment headers request received.");

                let node_client = node_client.clone();

                async move {
                    let internal_result = match req {
                        SuperSegmentHeaderRequest::SuperSegmentIndices {
                            super_segment_indices,
                        } => {
                            let super_segment_indices = Arc::unwrap_or_clone(super_segment_indices);

                            if super_segment_indices.len() > SUPER_SEGMENT_HEADERS_LIMIT as usize {
                                debug!(
                                    %peer_id,
                                    super_segment_indexes_count = %super_segment_indices.len(),
                                    %SUPER_SEGMENT_HEADERS_LIMIT,
                                    first_super_segment_index = ?super_segment_indices.first(),
                                    last_super_segment_index = ?super_segment_indices.last(),
                                    "`super_segment_indices` length exceed the limit",
                                );

                                return None;
                            }

                            debug!(
                                %peer_id,
                                super_segment_indexes_count = %super_segment_indices.len(),
                                first_super_segment_index = ?super_segment_indices.first(),
                                last_super_segment_index = ?super_segment_indices.last(),
                                "Super segment headers request received",
                            );

                            // To avoid remote denial of service, we don't update the cache in
                            // response to any network requests
                            node_client
                                .cached_super_segment_headers(super_segment_indices)
                                .await
                        }
                        SuperSegmentHeaderRequest::LastSuperSegmentHeaders { mut limit } => {
                            if limit > SUPER_SEGMENT_HEADERS_LIMIT {
                                debug!(
                                    %peer_id,
                                    %limit,
                                    %SUPER_SEGMENT_HEADERS_LIMIT,
                                    "Super segment header number exceeded the limit",
                                );

                                limit = SUPER_SEGMENT_HEADERS_LIMIT;
                            }
                            node_client.last_super_segment_headers(limit).await
                        }
                    };

                    match internal_result {
                        Ok(super_segment_headers) => super_segment_headers
                            .into_iter()
                            .inspect(|maybe_segment_header| {
                                if maybe_segment_header.is_none() {
                                    error!("Received empty optional super segment header!");
                                }
                            })
                            .collect::<Option<Vec<_>>>()
                            .map(|super_segment_headers| SuperSegmentHeaderResponse {
                                super_segment_headers,
                            }),
                        Err(error) => {
                            error!(%error, "Failed to get super segment headers from cache");

                            None
                        }
                    }
                }
                .in_current_span()
            }),
        ],
        max_established_outgoing_connections: out_connections,
        max_pending_outgoing_connections: pending_out_connections,
        max_established_incoming_connections: in_connections,
        max_pending_incoming_connections: pending_in_connections,
        bootstrap_addresses: bootstrap_nodes,
        kademlia_mode: KademliaMode::Dynamic,
        external_addresses,
        ..default_config
    };

    let (node, node_runner) = construct(config)?;
    maybe_weak_node.lock().replace(node.downgrade());

    node.on_new_listener(Arc::new({
        let node = node.clone();

        move |address| {
            info!(
                "DSN listening on {}",
                address.clone().with(Protocol::P2p(node.id()))
            );
        }
    }))
    .detach();

    // Consider returning HandlerId instead of each `detach()` calls for other usages.
    Ok((node, node_runner))
}
