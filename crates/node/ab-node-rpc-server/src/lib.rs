//! RPC API for the farmer

use ab_archiving::archiver::NewArchivedSegment;
use ab_client_api::{BeaconChainInfo, ChainSyncStatus};
use ab_client_archiving::recreate::{
    RecreateSegmentError, RecreateSegmentSuperSegmentDetails, recreate_genesis_segment,
    recreate_segment,
};
use ab_client_block_authoring::slot_worker::{
    BlockSealNotification, NewSlotInfo, NewSlotNotification,
};
use ab_client_consensus_common::ConsensusConstants;
use ab_core_primitives::block::header::OwnedBlockHeaderSeal;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::pot::SlotNumber;
use ab_core_primitives::segments::{
    HistorySize, LocalSegmentIndex, SegmentIndex, SuperSegment, SuperSegmentHeader,
    SuperSegmentIndex, SuperSegmentRoot,
};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::solutions::Solution;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo,
    MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST, SHARD_MEMBERSHIP_EXPIRATION, SlotInfo, SolutionResponse,
};
use ab_networking::libp2p::Multiaddr;
use async_lock::Mutex as AsyncMutex;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, SinkExt, StreamExt, select};
use jsonrpsee::core::{SubscriptionResult, async_trait};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{Server, ServerConfig};
use jsonrpsee::tokio::task::{JoinError, spawn_blocking};
use jsonrpsee::tokio::time::MissedTickBehavior;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use jsonrpsee::{
    ConnectionId, Extensions, PendingSubscriptionSink, SubscriptionSink, TrySendError,
};
use parking_lot::Mutex;
use schnellru::{ByLength, LruMap};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const CACHED_SUPER_SEGMENTS_CAPACITY: usize = 5;
const CACHED_ARCHIVED_SEGMENT_TIMEOUT: Duration = Duration::from_mins(1);

/// Top-level error type for the RPC handler.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Solution was ignored
    #[error("Solution was ignored for slot {slot}")]
    SolutionWasIgnored {
        /// Slot number
        slot: SlotNumber,
    },
    /// Super segment headers length exceeded the limit
    #[error(
        "Super segment headers length exceeded the limit: \
        {actual}/{MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST}"
    )]
    SuperSegmentHeadersLengthExceeded {
        /// Requested number of super segment headers/indices
        actual: usize,
    },
    /// Failed to recreate segment
    #[error("Failed to recreate segment: {0}")]
    FailedToRecreateSegment(#[from] RecreateSegmentError),
    /// Blocking task join error
    #[error("Blocking task join error: {0}")]
    BlockingTaskJoinError(#[from] JoinError),
}

impl From<Error> for ErrorObjectOwned {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::SolutionWasIgnored { .. } => 0,
            Error::SuperSegmentHeadersLengthExceeded { .. } => 1,
            Error::FailedToRecreateSegment(_) => 2,
            Error::BlockingTaskJoinError(_) => 3,
        };

        ErrorObject::owned(code, error.to_string(), None::<()>)
    }
}

/// Provides rpc methods for interacting with the farmer
#[rpc(server)]
pub trait FarmerRpcApi {
    /// Get metadata necessary for farmer operation
    #[method(name = "getFarmerAppInfo")]
    fn get_farmer_app_info(&self) -> Result<FarmerAppInfo, Error>;

    #[method(name = "submitSolutionResponse")]
    fn submit_solution_response(&self, solution_response: SolutionResponse) -> Result<(), Error>;

    /// Slot info subscription
    #[subscription(
        name = "subscribeSlotInfo" => "slot_info",
        unsubscribe = "unsubscribeSlotInfo",
        item = SlotInfo,
    )]
    async fn subscribe_slot_info(&self) -> SubscriptionResult;

    /// Sign block subscription
    #[subscription(
        name = "subscribeBlockSealing" => "block_seal",
        unsubscribe = "unsubscribeBlockSealing",
        item = BlockSealInfo,
    )]
    async fn subscribe_block_seal(&self) -> SubscriptionResult;

    #[method(name = "submitBlockSeal")]
    fn submit_block_seal(&self, block_seal: BlockSealResponse) -> Result<(), Error>;

    /// New super segment header subscription
    #[subscription(
        name = "subscribeNewSuperSegmentHeader" => "new_super_segment_header",
        unsubscribe = "unsubscribeNewSuperSegmentHeader",
        item = SuperSegmentHeader,
    )]
    async fn subscribe_new_super_segment_header(&self) -> SubscriptionResult;

    #[method(name = "superSegmentHeaders")]
    async fn super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> Result<Vec<Option<SuperSegmentHeader>>, Error>;

    #[method(name = "lastSuperSegmentHeaders")]
    async fn last_super_segment_headers(
        &self,
        limit: u32,
    ) -> Result<Vec<Option<SuperSegmentHeader>>, Error>;

    #[method(name = "superSegmentRootForSegmentIndex")]
    async fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> Result<Option<SuperSegmentRoot>, Error>;

    #[method(name = "piece")]
    async fn piece(&self, piece_index: PieceIndex) -> Result<Option<Piece>, Error>;

    #[method(name = "updateShardMembershipInfo", with_extensions)]
    async fn update_shard_membership_info(
        &self,
        info: Vec<FarmerShardMembershipInfo>,
    ) -> Result<(), Error>;
}

#[derive(Debug, Default)]
struct BlockSignatureSenders {
    current_pre_seal_hash: Blake3Hash,
    senders: Vec<oneshot::Sender<OwnedBlockHeaderSeal>>,
}

#[derive(Debug)]
struct CachedSuperSegments {
    super_segments: VecDeque<SuperSegment>,
}

impl Default for CachedSuperSegments {
    fn default() -> Self {
        Self {
            super_segments: VecDeque::with_capacity(CACHED_SUPER_SEGMENTS_CAPACITY),
        }
    }
}

impl CachedSuperSegments {
    fn get_for_segment_index(&self, segment_index: SegmentIndex) -> Option<&SuperSegment> {
        self.super_segments.iter().find(|super_segment| {
            let max_segment_index = super_segment.header.max_segment_index.as_inner();
            let first_segment_index = max_segment_index
                - SegmentIndex::from(u64::from(super_segment.header.num_segments))
                + SegmentIndex::ONE;

            (first_segment_index..=max_segment_index).contains(&segment_index)
        })
    }

    fn add(&mut self, super_segment: SuperSegment) {
        if self.super_segments.len() == CACHED_SUPER_SEGMENTS_CAPACITY {
            self.super_segments.pop_front();
        }

        self.super_segments.push_back(super_segment);
    }
}

/// Temporary in-memory cache of the last archived segment
#[derive(Debug)]
struct CachedArchivedSegment {
    segment_index: SegmentIndex,
    segment: NewArchivedSegment,
    last_used_at: Instant,
}

#[derive(Debug)]
struct ShardMembershipConnectionsState {
    last_update: Instant,
    info: Vec<FarmerShardMembershipInfo>,
}

#[derive(Debug, Default)]
struct ShardMembershipConnections {
    connections: HashMap<ConnectionId, ShardMembershipConnectionsState>,
}

/// Farmer RPC configuration
#[derive(Debug)]
pub struct FarmerRpcConfig<BCI, CSS> {
    /// IP and port (TCP) on which to listen for farmer RPC requests
    pub listen_on: SocketAddr,
    /// Genesis beacon chain block
    pub genesis_block: OwnedBeaconChainBlock,
    /// Consensus constants
    pub consensus_constants: ConsensusConstants,
    /// Max pieces in a sector
    pub max_pieces_in_sector: u16,
    /// New slot notifications
    pub new_slot_notification_receiver: mpsc::Receiver<NewSlotNotification>,
    /// Block sealing notifications
    pub block_sealing_notification_receiver: mpsc::Receiver<BlockSealNotification>,
    /// Super segment notifications
    pub new_super_segment_notification_receiver: mpsc::Receiver<SuperSegment>,
    /// Shard membership updates
    pub shard_membership_updates_sender: mpsc::Sender<Vec<FarmerShardMembershipInfo>>,
    /// DSN bootstrap nodes
    pub dsn_bootstrap_nodes: Vec<Multiaddr>,
    /// Beacon chain info
    pub beacon_chain_info: BCI,
    /// Chain sync status
    pub chain_sync_status: CSS,
    /// Erasure coding instance
    pub erasure_coding: ErasureCoding,
}

/// Worker that drives RPC server tasks
#[derive(Debug)]
pub struct FarmerRpcWorker<BCI, CSS>
where
    BCI: BeaconChainInfo,
    CSS: ChainSyncStatus,
{
    server: Option<Server>,
    rpc: Option<FarmerRpc<BCI, CSS>>,
    new_slot_notification_receiver: mpsc::Receiver<NewSlotNotification>,
    block_sealing_notification_receiver: mpsc::Receiver<BlockSealNotification>,
    new_super_segment_notification_receiver: mpsc::Receiver<SuperSegment>,
    solution_response_senders: Arc<Mutex<LruMap<SlotNumber, mpsc::Sender<Solution>>>>,
    block_sealing_senders: Arc<Mutex<BlockSignatureSenders>>,
    slot_info_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    block_sealing_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    new_super_segment_header_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    cached_archived_segment: Arc<AsyncMutex<Option<CachedArchivedSegment>>>,
    cached_super_segments: Arc<Mutex<CachedSuperSegments>>,
}

impl<BCI, CSS> FarmerRpcWorker<BCI, CSS>
where
    BCI: BeaconChainInfo,
    CSS: ChainSyncStatus,
{
    /// Creates a new farmer RPC worker
    pub async fn new(config: FarmerRpcConfig<BCI, CSS>) -> io::Result<Self> {
        let server = Server::builder()
            .set_config(ServerConfig::builder().ws_only().build())
            .build(config.listen_on)
            .await?;

        let address = server.local_addr()?;
        info!(%address, "Started farmer RPC server");

        let block_authoring_delay = u64::from(config.consensus_constants.block_authoring_delay);
        let block_authoring_delay = usize::try_from(block_authoring_delay)
            .expect("Block authoring delay will never exceed usize on any platform; qed");
        let solution_response_senders_capacity = u32::try_from(block_authoring_delay)
            .expect("Always a tiny constant in the protocol; qed");

        let slot_info_subscriptions = Arc::default();
        let block_sealing_subscriptions = Arc::default();

        let solution_response_senders = Arc::new(Mutex::new(LruMap::new(ByLength::new(
            solution_response_senders_capacity,
        ))));
        let block_sealing_senders = Arc::default();
        let new_super_segment_header_subscriptions = Arc::default();
        let cached_archived_segment = Arc::default();
        let cached_super_segments = Arc::default();

        let rpc = FarmerRpc {
            genesis_block: config.genesis_block,
            solution_response_senders: Arc::clone(&solution_response_senders),
            block_sealing_senders: Arc::clone(&block_sealing_senders),
            dsn_bootstrap_nodes: config.dsn_bootstrap_nodes,
            beacon_chain_info: config.beacon_chain_info,
            chain_sync_status: config.chain_sync_status,
            consensus_constants: config.consensus_constants,
            max_pieces_in_sector: config.max_pieces_in_sector,
            slot_info_subscriptions: Arc::clone(&slot_info_subscriptions),
            block_sealing_subscriptions: Arc::clone(&block_sealing_subscriptions),
            new_super_segment_header_subscriptions: Arc::clone(
                &new_super_segment_header_subscriptions,
            ),
            cached_archived_segment: Arc::clone(&cached_archived_segment),
            cached_super_segments: Arc::clone(&cached_super_segments),
            shard_membership_connections: Arc::default(),
            shard_membership_updates_sender: config.shard_membership_updates_sender,
            erasure_coding: config.erasure_coding,
        };

        Ok(Self {
            server: Some(server),
            rpc: Some(rpc),
            new_slot_notification_receiver: config.new_slot_notification_receiver,
            block_sealing_notification_receiver: config.block_sealing_notification_receiver,
            new_super_segment_notification_receiver: config.new_super_segment_notification_receiver,
            solution_response_senders,
            block_sealing_senders,
            slot_info_subscriptions,
            block_sealing_subscriptions,
            new_super_segment_header_subscriptions,
            cached_archived_segment,
            cached_super_segments,
        })
    }

    /// Drive RPC server tasks
    pub async fn run(mut self) {
        let server = self.server.take().expect("Called only once from here; qed");
        let rpc = self.rpc.take().expect("Called only once from here; qed");
        let mut server_fut = server.start(rpc.into_rpc()).stopped().boxed().fuse();

        // Also send periodic updates in addition to the subscription response
        let mut archived_segment_cache_cleanup_interval =
            tokio::time::interval(CACHED_ARCHIVED_SEGMENT_TIMEOUT);
        archived_segment_cache_cleanup_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            select! {
                _ = server_fut => {}
                maybe_new_slot_notification = self.new_slot_notification_receiver.next() => {
                    let Some(new_slot_notification) = maybe_new_slot_notification else {
                        break;
                    };

                    self.handle_new_slot_notification(new_slot_notification).await;
                }
                maybe_block_sealing_notification = self.block_sealing_notification_receiver.next() => {
                    let Some(block_sealing_notification) = maybe_block_sealing_notification else {
                        break;
                    };

                    self.handle_block_sealing_notification(block_sealing_notification).await;
                }
                maybe_new_super_segment = self.new_super_segment_notification_receiver.next() => {
                    let Some(new_super_segment) = maybe_new_super_segment else {
                        break;
                    };

                    self.handle_new_super_segment(new_super_segment).await;
                }
                _ = archived_segment_cache_cleanup_interval.tick().fuse() => {
                    if let Some(mut maybe_cached_archived_segment) = self.cached_archived_segment.try_lock()
                        && let Some(cached_archived_segment) = maybe_cached_archived_segment.as_ref()
                        && cached_archived_segment.last_used_at.elapsed() >= CACHED_ARCHIVED_SEGMENT_TIMEOUT
                    {
                        maybe_cached_archived_segment.take();
                    }
                }
            }
        }
    }

    async fn handle_new_slot_notification(&mut self, new_slot_notification: NewSlotNotification) {
        let NewSlotNotification {
            new_slot_info,
            solution_sender,
        } = new_slot_notification;

        let NewSlotInfo {
            slot,
            proof_of_time,
            solution_range,
            shard_membership_entropy,
            num_shards,
        } = new_slot_info;

        // Store solution sender so that we can retrieve it when solution comes from
        // the farmer
        let mut solution_response_senders = self.solution_response_senders.lock();
        if solution_response_senders.peek(&slot).is_none() {
            solution_response_senders.insert(slot, solution_sender);
        }

        let global_challenge = proof_of_time.derive_global_challenge(slot);

        // This will be sent to the farmer
        let slot_info = SlotInfo {
            slot,
            global_challenge,
            solution_range: solution_range.to_leaf_shard(num_shards),
            shard_membership_entropy,
            num_shards,
        };
        let slot_info = serde_json::value::to_raw_value(&slot_info)
            .expect("Serialization of slot info never fails; qed");

        self.slot_info_subscriptions.lock().retain_mut(|sink| {
            match sink.try_send(slot_info.clone()) {
                Ok(()) => true,
                Err(error) => match error {
                    TrySendError::Closed(_) => {
                        // Remove closed receivers
                        false
                    }
                    TrySendError::Full(_) => {
                        warn!(
                            subscription_id = ?sink.subscription_id(),
                            "Slot info receiver is too slow, dropping notification"
                        );
                        true
                    }
                },
            }
        });
    }

    async fn handle_block_sealing_notification(
        &mut self,
        block_sealing_notification: BlockSealNotification,
    ) {
        let BlockSealNotification {
            pre_seal_hash,
            public_key_hash,
            seal_sender,
        } = block_sealing_notification;

        // Store signature sender so that we can retrieve it when a solution comes from the farmer
        {
            let mut block_sealing_senders = self.block_sealing_senders.lock();

            if block_sealing_senders.current_pre_seal_hash != pre_seal_hash {
                block_sealing_senders.current_pre_seal_hash = pre_seal_hash;
                block_sealing_senders.senders.clear();
            }

            block_sealing_senders.senders.push(seal_sender);
        }

        // This will be sent to the farmer
        let block_seal_info = BlockSealInfo {
            pre_seal_hash,
            public_key_hash,
        };
        let block_seal_info = serde_json::value::to_raw_value(&block_seal_info)
            .expect("Serialization of block seal info never fails; qed");

        self.block_sealing_subscriptions.lock().retain_mut(|sink| {
            match sink.try_send(block_seal_info.clone()) {
                Ok(()) => true,
                Err(error) => match error {
                    TrySendError::Closed(_) => {
                        // Remove closed receivers
                        false
                    }
                    TrySendError::Full(_) => {
                        warn!(
                            subscription_id = ?sink.subscription_id(),
                            "Block seal info receiver is too slow, dropping notification"
                        );
                        true
                    }
                },
            }
        });
    }

    async fn handle_new_super_segment(&mut self, super_segment: SuperSegment) {
        // This will be sent to the farmer
        let super_segment_header = serde_json::value::to_raw_value(&super_segment.header)
            .expect("Serialization of super segment info never fails; qed");

        self.cached_super_segments.lock().add(super_segment);

        self.new_super_segment_header_subscriptions
            .lock()
            .retain_mut(|sink| {
                let subscription_id = sink.subscription_id();

                match sink.try_send(super_segment_header.clone()) {
                    Ok(()) => true,
                    Err(error) => match error {
                        TrySendError::Closed(_) => false,
                        TrySendError::Full(_) => {
                            warn!(
                                ?subscription_id,
                                "Super segment receiver is too slow, dropping notification"
                            );
                            true
                        }
                    },
                }
            });
    }
}

/// Implements the [`FarmerRpcApiServer`] trait for a farmer to connect to
#[derive(Debug)]
struct FarmerRpc<BCI, CSS>
where
    BCI: BeaconChainInfo,
    CSS: ChainSyncStatus,
{
    genesis_block: OwnedBeaconChainBlock,
    solution_response_senders: Arc<Mutex<LruMap<SlotNumber, mpsc::Sender<Solution>>>>,
    block_sealing_senders: Arc<Mutex<BlockSignatureSenders>>,
    dsn_bootstrap_nodes: Vec<Multiaddr>,
    beacon_chain_info: BCI,
    chain_sync_status: CSS,
    consensus_constants: ConsensusConstants,
    max_pieces_in_sector: u16,
    slot_info_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    block_sealing_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    new_super_segment_header_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    cached_archived_segment: Arc<AsyncMutex<Option<CachedArchivedSegment>>>,
    cached_super_segments: Arc<Mutex<CachedSuperSegments>>,
    shard_membership_connections: Arc<Mutex<ShardMembershipConnections>>,
    shard_membership_updates_sender: mpsc::Sender<Vec<FarmerShardMembershipInfo>>,
    erasure_coding: ErasureCoding,
}

#[async_trait]
impl<BCI, CSS> FarmerRpcApiServer for FarmerRpc<BCI, CSS>
where
    BCI: BeaconChainInfo,
    CSS: ChainSyncStatus,
{
    fn get_farmer_app_info(&self) -> Result<FarmerAppInfo, Error> {
        let last_segment_index = self
            .beacon_chain_info
            .last_segment_header()
            .map(|segment_header| segment_header.segment_index.as_inner())
            .unwrap_or(LocalSegmentIndex::ZERO);

        let consensus_constants = &self.consensus_constants;
        let protocol_info = FarmerProtocolInfo {
            history_size: HistorySize::from(SegmentIndex::from(last_segment_index)),
            max_pieces_in_sector: self.max_pieces_in_sector,
            recent_segments: consensus_constants.recent_segments,
            recent_history_fraction: consensus_constants.recent_history_fraction,
            min_sector_lifetime: consensus_constants.min_sector_lifetime,
        };

        let farmer_app_info = FarmerAppInfo {
            genesis_root: *self.genesis_block.header.header().root(),
            dsn_bootstrap_nodes: self.dsn_bootstrap_nodes.clone(),
            syncing: self.chain_sync_status.is_syncing(),
            farming_timeout: consensus_constants
                .slot_duration
                .as_duration()
                .mul_f64(u64::from(consensus_constants.block_authoring_delay) as f64),
            protocol_info,
        };

        Ok(farmer_app_info)
    }

    fn submit_solution_response(&self, solution_response: SolutionResponse) -> Result<(), Error> {
        let slot = solution_response.slot_number;
        let public_key_hash = solution_response.solution.public_key_hash;
        let sector_index = solution_response.solution.sector_index;
        let mut solution_response_senders = self.solution_response_senders.lock();

        let success = solution_response_senders
            .peek_mut(&slot)
            .and_then(|sender| sender.try_send(solution_response.solution).ok())
            .is_some();

        if !success {
            warn!(
                %slot,
                %sector_index,
                %public_key_hash,
                "Solution was ignored, likely because farmer was too slow"
            );

            return Err(Error::SolutionWasIgnored { slot });
        }

        Ok(())
    }

    async fn subscribe_slot_info(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let subscription = pending.accept().await?;
        self.slot_info_subscriptions.lock().push(subscription);

        Ok(())
    }

    async fn subscribe_block_seal(&self, pending: PendingSubscriptionSink) -> SubscriptionResult {
        let subscription = pending.accept().await?;
        self.block_sealing_subscriptions.lock().push(subscription);

        Ok(())
    }

    fn submit_block_seal(&self, block_seal: BlockSealResponse) -> Result<(), Error> {
        let block_sealing_senders = self.block_sealing_senders.clone();

        let mut block_sealing_senders = block_sealing_senders.lock();

        if block_sealing_senders.current_pre_seal_hash == block_seal.pre_seal_hash
            && let Some(sender) = block_sealing_senders.senders.pop()
        {
            let _ = sender.send(block_seal.seal);
        }

        Ok(())
    }

    async fn subscribe_new_super_segment_header(
        &self,
        pending: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let subscription = pending.accept().await?;
        self.new_super_segment_header_subscriptions
            .lock()
            .push(subscription);

        Ok(())
    }

    async fn super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> Result<Vec<Option<SuperSegmentHeader>>, Error> {
        if super_segment_indices.len() > MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "`super_segment_indices` length exceed the limit: {} ",
                super_segment_indices.len()
            );

            return Err(Error::SuperSegmentHeadersLengthExceeded {
                actual: super_segment_indices.len(),
            });
        };

        Ok(super_segment_indices
            .into_iter()
            .map(|super_segment_index| {
                self.beacon_chain_info
                    .get_super_segment_header(super_segment_index)
            })
            .collect())
    }

    async fn last_super_segment_headers(
        &self,
        limit: u32,
    ) -> Result<Vec<Option<SuperSegmentHeader>>, Error> {
        if limit as usize > MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "Request limit ({}) exceed the server limit: {} ",
                limit, MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST
            );

            return Err(Error::SuperSegmentHeadersLengthExceeded {
                actual: limit as usize,
            });
        };

        let last_super_segment_index = self
            .beacon_chain_info
            .last_super_segment_header()
            .map(|super_segment_header| super_segment_header.index.as_inner())
            .unwrap_or(SuperSegmentIndex::ZERO);

        let mut last_super_segment_headers = (SuperSegmentIndex::ZERO..=last_super_segment_index)
            .rev()
            .take(limit as usize)
            .map(|super_segment_index| {
                self.beacon_chain_info
                    .get_super_segment_header(super_segment_index)
            })
            .collect::<Vec<_>>();

        last_super_segment_headers.reverse();

        Ok(last_super_segment_headers)
    }

    async fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> Result<Option<SuperSegmentRoot>, Error> {
        Ok(self
            .beacon_chain_info
            .get_super_segment_header_for_segment_index(segment_index)
            .map(|super_segment_header| super_segment_header.root))
    }

    // Note: this RPC uses the cached archived segment, which is only updated by archived segments
    // subscriptions
    async fn piece(&self, requested_piece_index: PieceIndex) -> Result<Option<Piece>, Error> {
        let segment_index = requested_piece_index.segment_index();
        let cached_archived_segment = &mut *self.cached_archived_segment.lock().await;

        if let Some(cached_archived_segment) = cached_archived_segment
            && cached_archived_segment.segment_index == segment_index
        {
            cached_archived_segment.last_used_at = Instant::now();

            return Ok(cached_archived_segment
                .segment
                .pieces
                .pieces()
                .nth(usize::from(requested_piece_index.position())));
        }

        if segment_index == SegmentIndex::ZERO {
            let segment = spawn_blocking({
                let genesis_block = self.genesis_block.clone();
                let erasure_coding = self.erasure_coding.clone();

                move || recreate_genesis_segment(&genesis_block, erasure_coding)
            })
            .await?;
            let cached_archived_segment = cached_archived_segment.insert(CachedArchivedSegment {
                segment_index: SegmentIndex::ZERO,
                segment,
                last_used_at: Instant::now(),
            });

            return Ok(cached_archived_segment
                .segment
                .pieces
                .pieces()
                .nth(usize::from(requested_piece_index.position())));
        }

        let (super_segment_index, shard_segment_root_with_position, segment_proof) = {
            let cached_super_segments = self.cached_super_segments.lock();
            let Some(super_segment) = cached_super_segments.get_for_segment_index(segment_index)
            else {
                return Ok(None);
            };

            let Some(shard_segment_root_with_position) = super_segment
                .segment_roots
                .iter()
                .nth_back(u64::from(
                    super_segment.header.max_segment_index.as_inner() - segment_index,
                ) as usize)
                .copied()
            else {
                error!(
                    %requested_piece_index,
                    %segment_index,
                    super_segment_header = ?super_segment.header,
                    "Failed to find segment index inside super segment, this should never happen"
                );
                return Ok(None);
            };

            let segment_position = shard_segment_root_with_position.segment_position;

            let Some(segment_proof) = super_segment.proof_for_segment(segment_position) else {
                error!(
                    %requested_piece_index,
                    %segment_index,
                    %segment_position,
                    super_segment_header = ?super_segment.header,
                    "Failed to get segment proof for segment position, this should never happen"
                );

                return Ok(None);
            };

            (
                super_segment.header.index.as_inner(),
                shard_segment_root_with_position,
                segment_proof,
            )
        };

        let recreate_segment_super_segment_details = RecreateSegmentSuperSegmentDetails {
            super_segment_index,
            segment_position: shard_segment_root_with_position.segment_position,
            segment_proof,
        };

        if shard_segment_root_with_position.shard_index != ShardIndex::BEACON_CHAIN {
            // TODO: There will be a need for chain info instances of all live shards to re-derive
            //  segments here, but there is just a beacon chain here for now
            unimplemented!("Shard segments for non-beacon chain shards are not supported yet");
        }

        let last_archived_segment = shard_segment_root_with_position
            .local_segment_index
            .checked_sub(LocalSegmentIndex::ONE)
            .and_then(|last_segment_index| {
                self.beacon_chain_info
                    .get_segment_header(last_segment_index)
            });

        let maybe_segment = recreate_segment(
            last_archived_segment,
            &self.beacon_chain_info,
            self.erasure_coding.clone(),
            &recreate_segment_super_segment_details,
            |_| Vec::new(),
        )
        .await?;

        let Some(segment) = maybe_segment else {
            return Ok(None);
        };

        let cached_archived_segment = cached_archived_segment.insert(CachedArchivedSegment {
            segment_index,
            segment,
            last_used_at: Instant::now(),
        });

        Ok(cached_archived_segment
            .segment
            .pieces
            .pieces()
            .nth(usize::from(requested_piece_index.position())))
    }

    async fn update_shard_membership_info(
        &self,
        extensions: &Extensions,
        info: Vec<FarmerShardMembershipInfo>,
    ) -> Result<(), Error> {
        let connection_id = extensions
            .get::<ConnectionId>()
            .expect("`ConnectionId` is always present; qed");

        let shard_membership = {
            let mut shard_membership_connections = self.shard_membership_connections.lock();

            // TODO: This is a workaround for https://github.com/paritytech/jsonrpsee/issues/1617
            //  and should be replaced with cleanup on disconnection once that issue is resolved
            shard_membership_connections
                .connections
                .retain(|_connection_id, state| {
                    state.last_update.elapsed() >= SHARD_MEMBERSHIP_EXPIRATION
                });

            shard_membership_connections.connections.insert(
                *connection_id,
                ShardMembershipConnectionsState {
                    last_update: Instant::now(),
                    info,
                },
            );

            shard_membership_connections
                .connections
                .values()
                .flat_map(|state| state.info.clone())
                .collect::<Vec<_>>()
        };

        if let Err(error) = self
            .shard_membership_updates_sender
            .clone()
            .send(shard_membership)
            .await
        {
            warn!(%error, "Failed to send shard membership update");
        }

        Ok(())
    }
}
