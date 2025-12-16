//! RPC API for the farmer

#![feature(try_blocks)]

use ab_archiving::archiver::NewArchivedSegment;
use ab_client_api::{ChainInfo, ChainSyncStatus};
use ab_client_archiving::{ArchivedSegmentNotification, recreate_genesis_segment};
use ab_client_block_authoring::slot_worker::{
    BlockSealNotification, NewSlotInfo, NewSlotNotification,
};
use ab_client_consensus_common::ConsensusConstants;
use ab_core_primitives::block::header::OwnedBlockHeaderSeal;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::pot::SlotNumber;
use ab_core_primitives::segments::{HistorySize, SegmentHeader, SegmentIndex};
use ab_core_primitives::solutions::Solution;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo,
    MAX_SEGMENT_HEADERS_PER_REQUEST, SHARD_MEMBERSHIP_EXPIRATION, SlotInfo, SolutionResponse,
};
use ab_networking::libp2p::Multiaddr;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, SinkExt, StreamExt, select};
use jsonrpsee::core::{SubscriptionResult, async_trait};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{Server, ServerConfig};
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned, SubscriptionId};
use jsonrpsee::{
    ConnectionId, Extensions, PendingSubscriptionSink, SubscriptionSink, TrySendError,
};
use parking_lot::Mutex;
use schnellru::{ByLength, LruMap};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Top-level error type for the RPC handler.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Solution was ignored
    #[error("Solution was ignored for slot {slot}")]
    SolutionWasIgnored {
        /// Slot number
        slot: SlotNumber,
    },
    /// Segment headers length exceeded the limit
    #[error(
        "Segment headers length exceeded the limit: {actual}/{MAX_SEGMENT_HEADERS_PER_REQUEST}"
    )]
    SegmentHeadersLengthExceeded {
        /// Requested number of segment headers/indices
        actual: usize,
    },
}

impl From<Error> for ErrorObjectOwned {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::SolutionWasIgnored { .. } => 0,
            Error::SegmentHeadersLengthExceeded { .. } => 1,
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

    /// Archived segment header subscription
    #[subscription(
        name = "subscribeArchivedSegmentHeader" => "archived_segment_header",
        unsubscribe = "unsubscribeArchivedSegmentHeader",
        item = SegmentHeader,
    )]
    async fn subscribe_archived_segment_header(&self) -> SubscriptionResult;

    #[method(name = "segmentHeaders")]
    async fn segment_headers(
        &self,
        segment_indices: Vec<SegmentIndex>,
    ) -> Result<Vec<Option<SegmentHeader>>, Error>;

    #[method(name = "piece", blocking)]
    fn piece(&self, piece_index: PieceIndex) -> Result<Option<Piece>, Error>;

    #[method(name = "acknowledgeArchivedSegmentHeader")]
    async fn acknowledge_archived_segment_header(
        &self,
        segment_index: SegmentIndex,
    ) -> Result<(), Error>;

    #[method(name = "lastSegmentHeaders")]
    async fn last_segment_headers(&self, limit: u32) -> Result<Vec<Option<SegmentHeader>>, Error>;

    #[method(name = "updateShardMembershipInfo", with_extensions)]
    async fn update_shard_membership_info(
        &self,
        info: Vec<FarmerShardMembershipInfo>,
    ) -> Result<(), Error>;
}

#[derive(Debug, Default)]
struct ArchivedSegmentHeaderAcknowledgementSenders {
    segment_index: SegmentIndex,
    senders: HashMap<SubscriptionId<'static>, mpsc::Sender<()>>,
}

#[derive(Debug, Default)]
struct BlockSignatureSenders {
    current_pre_seal_hash: Blake3Hash,
    senders: Vec<oneshot::Sender<OwnedBlockHeaderSeal>>,
}

/// In-memory cache of last archived segment, such that when request comes back right after
/// archived segment notification, RPC server is able to answer quickly.
///
/// We store weak reference, such that archived segment is not persisted for longer than
/// necessary occupying RAM.
#[derive(Debug)]
enum CachedArchivedSegment {
    /// Special case for genesis segment when requested over RPC
    Genesis(Arc<NewArchivedSegment>),
    Weak(Weak<NewArchivedSegment>),
}

impl CachedArchivedSegment {
    fn get(&self) -> Option<Arc<NewArchivedSegment>> {
        match self {
            CachedArchivedSegment::Genesis(archived_segment) => Some(Arc::clone(archived_segment)),
            CachedArchivedSegment::Weak(weak_archived_segment) => weak_archived_segment.upgrade(),
        }
    }
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
pub struct FarmerRpcConfig<CI, CSS> {
    /// IP and port (TCP) on which to listen for farmer RPC requests
    pub listen_on: SocketAddr,
    /// Genesis beacon beacon chain block
    pub genesis_block: OwnedBeaconChainBlock,
    /// Consensus constants
    pub consensus_constants: ConsensusConstants,
    /// Max pieces in sector
    pub max_pieces_in_sector: u16,
    /// New slot notifications
    pub new_slot_notification_receiver: mpsc::Receiver<NewSlotNotification>,
    /// Block sealing notifications
    pub block_sealing_notification_receiver: mpsc::Receiver<BlockSealNotification>,
    /// Archived segment notifications
    pub archived_segment_notification_receiver: mpsc::Receiver<ArchivedSegmentNotification>,
    /// Shard membership updates
    pub shard_membership_updates_sender: mpsc::Sender<Vec<FarmerShardMembershipInfo>>,
    /// DSN bootstrap nodes
    pub dsn_bootstrap_nodes: Vec<Multiaddr>,
    /// Beacon chain info
    pub chain_info: CI,
    /// Chain sync status
    pub chain_sync_status: CSS,
    /// Erasure coding instance
    pub erasure_coding: ErasureCoding,
}

/// Worker that drives RPC server tasks
#[derive(Debug)]
pub struct FarmerRpcWorker<CI, CSS>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    server: Option<Server>,
    rpc: Option<FarmerRpc<CI, CSS>>,
    new_slot_notification_receiver: mpsc::Receiver<NewSlotNotification>,
    block_sealing_notification_receiver: mpsc::Receiver<BlockSealNotification>,
    archived_segment_notification_receiver: mpsc::Receiver<ArchivedSegmentNotification>,
    solution_response_senders: Arc<Mutex<LruMap<SlotNumber, mpsc::Sender<Solution>>>>,
    block_sealing_senders: Arc<Mutex<BlockSignatureSenders>>,
    cached_archived_segment: Arc<Mutex<Option<CachedArchivedSegment>>>,
    archived_segment_acknowledgement_senders:
        Arc<Mutex<ArchivedSegmentHeaderAcknowledgementSenders>>,
    slot_info_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    block_sealing_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    archived_segment_header_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
}

impl<CI, CSS> FarmerRpcWorker<CI, CSS>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    /// Creates a new farmer RPC worker
    pub async fn new(config: FarmerRpcConfig<CI, CSS>) -> io::Result<Self> {
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
        let cached_archived_segment = Arc::default();
        let archived_segment_header_subscriptions = Arc::default();

        let rpc = FarmerRpc {
            genesis_block: config.genesis_block,
            solution_response_senders: Arc::clone(&solution_response_senders),
            block_sealing_senders: Arc::clone(&block_sealing_senders),
            dsn_bootstrap_nodes: config.dsn_bootstrap_nodes,
            chain_info: config.chain_info,
            cached_archived_segment: Arc::clone(&cached_archived_segment),
            archived_segment_acknowledgement_senders: Arc::default(),
            chain_sync_status: config.chain_sync_status,
            consensus_constants: config.consensus_constants,
            max_pieces_in_sector: config.max_pieces_in_sector,
            slot_info_subscriptions: Arc::clone(&slot_info_subscriptions),
            block_sealing_subscriptions: Arc::clone(&block_sealing_subscriptions),
            archived_segment_header_subscriptions: Arc::clone(
                &archived_segment_header_subscriptions,
            ),
            shard_membership_connections: Arc::default(),
            shard_membership_updates_sender: config.shard_membership_updates_sender,
            erasure_coding: config.erasure_coding,
        };

        Ok(Self {
            server: Some(server),
            rpc: Some(rpc),
            new_slot_notification_receiver: config.new_slot_notification_receiver,
            block_sealing_notification_receiver: config.block_sealing_notification_receiver,
            archived_segment_notification_receiver: config.archived_segment_notification_receiver,
            solution_response_senders,
            block_sealing_senders,
            cached_archived_segment,
            archived_segment_acknowledgement_senders: Arc::new(Default::default()),
            slot_info_subscriptions,
            block_sealing_subscriptions,
            archived_segment_header_subscriptions,
        })
    }

    /// Drive RPC server tasks
    pub async fn run(mut self) {
        let server = self.server.take().expect("Called only once from here; qed");
        let rpc = self.rpc.take().expect("Called only once from here; qed");
        let mut server_fut = server.start(rpc.into_rpc()).stopped().boxed().fuse();

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
                maybe_archived_segment_notification = self.archived_segment_notification_receiver.next() => {
                    let Some(archived_segment_notification) = maybe_archived_segment_notification else {
                        break;
                    };

                    self.handle_archived_segment_notification(archived_segment_notification).await;
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
            solution_range: solution_range.to_farmer_solution_range(num_shards),
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

    async fn handle_archived_segment_notification(
        &mut self,
        archived_segment_notification: ArchivedSegmentNotification,
    ) {
        let ArchivedSegmentNotification {
            archived_segment,
            acknowledgement_sender,
        } = archived_segment_notification;

        let segment_index = archived_segment.segment_header.segment_index();

        // TODO: Combine `archived_segment_header_subscriptions` and
        //  `archived_segment_acknowledgement_senders` under the same lock to avoid potential
        //  accidental deadlock with future changed
        self.archived_segment_header_subscriptions
            .lock()
            .retain_mut(|sink| {
                let subscription_id = sink.subscription_id();

                // Store acknowledgment sender so that we can retrieve it when acknowledgment
                // comes from the farmer
                let mut archived_segment_acknowledgement_senders =
                    self.archived_segment_acknowledgement_senders.lock();

                if archived_segment_acknowledgement_senders.segment_index != segment_index {
                    archived_segment_acknowledgement_senders.segment_index = segment_index;
                    archived_segment_acknowledgement_senders.senders.clear();
                }

                let maybe_archived_segment_header = match archived_segment_acknowledgement_senders
                    .senders
                    .entry(subscription_id.clone())
                {
                    Entry::Occupied(_) => {
                        // No need to do anything, a farmer is processing a request
                        None
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(acknowledgement_sender.clone());

                        // This will be sent to the farmer
                        Some(archived_segment.segment_header)
                    }
                };

                self.cached_archived_segment
                    .lock()
                    .replace(CachedArchivedSegment::Weak(Arc::downgrade(
                        &archived_segment,
                    )));

                // This will be sent to the farmer
                let maybe_archived_segment_header =
                    serde_json::value::to_raw_value(&maybe_archived_segment_header)
                        .expect("Serialization of archived segment info never fails; qed");

                match sink.try_send(maybe_archived_segment_header) {
                    Ok(()) => true,
                    Err(error) => match error {
                        TrySendError::Closed(_) => {
                            // Remove closed receivers
                            archived_segment_acknowledgement_senders
                                .senders
                                .remove(&subscription_id);
                            false
                        }
                        TrySendError::Full(_) => {
                            warn!(
                                ?subscription_id,
                                "Block seal info receiver is too slow, dropping notification"
                            );
                            true
                        }
                    },
                }
            });
    }
}

/// Implements the [`FarmerRpcApiServer`] trait for farmer to connect to
#[derive(Debug)]
struct FarmerRpc<CI, CSS>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    genesis_block: OwnedBeaconChainBlock,
    solution_response_senders: Arc<Mutex<LruMap<SlotNumber, mpsc::Sender<Solution>>>>,
    block_sealing_senders: Arc<Mutex<BlockSignatureSenders>>,
    dsn_bootstrap_nodes: Vec<Multiaddr>,
    chain_info: CI,
    cached_archived_segment: Arc<Mutex<Option<CachedArchivedSegment>>>,
    archived_segment_acknowledgement_senders:
        Arc<Mutex<ArchivedSegmentHeaderAcknowledgementSenders>>,
    chain_sync_status: CSS,
    consensus_constants: ConsensusConstants,
    max_pieces_in_sector: u16,
    slot_info_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    block_sealing_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    archived_segment_header_subscriptions: Arc<Mutex<Vec<SubscriptionSink>>>,
    shard_membership_connections: Arc<Mutex<ShardMembershipConnections>>,
    shard_membership_updates_sender: mpsc::Sender<Vec<FarmerShardMembershipInfo>>,
    erasure_coding: ErasureCoding,
}

#[async_trait]
impl<CI, CSS> FarmerRpcApiServer for FarmerRpc<CI, CSS>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    fn get_farmer_app_info(&self) -> Result<FarmerAppInfo, Error> {
        let last_segment_index = self
            .chain_info
            .max_segment_index()
            .unwrap_or(SegmentIndex::ZERO);

        let consensus_constants = &self.consensus_constants;
        let protocol_info = FarmerProtocolInfo {
            history_size: HistorySize::from(last_segment_index),
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
                .mul_f64(consensus_constants.block_authoring_delay.as_u64() as f64),
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

    async fn subscribe_archived_segment_header(
        &self,
        pending: PendingSubscriptionSink,
    ) -> SubscriptionResult {
        let subscription = pending.accept().await?;
        self.archived_segment_header_subscriptions
            .lock()
            .push(subscription);

        Ok(())
    }

    async fn acknowledge_archived_segment_header(
        &self,
        segment_index: SegmentIndex,
    ) -> Result<(), Error> {
        let archived_segment_acknowledgement_senders =
            self.archived_segment_acknowledgement_senders.clone();

        let maybe_sender = {
            let mut archived_segment_acknowledgement_senders_guard =
                archived_segment_acknowledgement_senders.lock();

            (archived_segment_acknowledgement_senders_guard.segment_index == segment_index)
                .then(|| {
                    let last_key = archived_segment_acknowledgement_senders_guard
                        .senders
                        .keys()
                        .next()
                        .cloned()?;

                    archived_segment_acknowledgement_senders_guard
                        .senders
                        .remove(&last_key)
                })
                .flatten()
        };

        if let Some(mut sender) = maybe_sender
            && let Err(error) = sender.try_send(())
            && !error.is_disconnected()
        {
            warn!(%error, "Failed to acknowledge archived segment");
        }

        debug!(%segment_index, "Acknowledged archived segment.");

        Ok(())
    }

    // Note: this RPC uses the cached archived segment, which is only updated by archived segments
    // subscriptions
    fn piece(&self, requested_piece_index: PieceIndex) -> Result<Option<Piece>, Error> {
        let archived_segment = {
            let mut cached_archived_segment = self.cached_archived_segment.lock();

            match cached_archived_segment
                .as_ref()
                .and_then(CachedArchivedSegment::get)
            {
                Some(archived_segment) => archived_segment,
                None => {
                    if requested_piece_index > SegmentIndex::ZERO.last_piece_index() {
                        return Ok(None);
                    }

                    debug!(%requested_piece_index, "Re-creating the genesis segment on demand");

                    // Re-create the genesis segment on demand
                    let archived_segment = Arc::new(recreate_genesis_segment(
                        &self.genesis_block,
                        self.erasure_coding.clone(),
                    ));

                    cached_archived_segment.replace(CachedArchivedSegment::Genesis(Arc::clone(
                        &archived_segment,
                    )));

                    archived_segment
                }
            }
        };

        if requested_piece_index.segment_index() == archived_segment.segment_header.segment_index()
        {
            return Ok(archived_segment
                .pieces
                .pieces()
                .nth(requested_piece_index.position() as usize));
        }

        Ok(None)
    }

    async fn segment_headers(
        &self,
        segment_indices: Vec<SegmentIndex>,
    ) -> Result<Vec<Option<SegmentHeader>>, Error> {
        if segment_indices.len() > MAX_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "`segment_indices` length exceed the limit: {} ",
                segment_indices.len()
            );

            return Err(Error::SegmentHeadersLengthExceeded {
                actual: segment_indices.len(),
            });
        };

        Ok(segment_indices
            .into_iter()
            .map(|segment_index| self.chain_info.get_segment_header(segment_index))
            .collect())
    }

    async fn last_segment_headers(&self, limit: u32) -> Result<Vec<Option<SegmentHeader>>, Error> {
        if limit as usize > MAX_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "Request limit ({}) exceed the server limit: {} ",
                limit, MAX_SEGMENT_HEADERS_PER_REQUEST
            );

            return Err(Error::SegmentHeadersLengthExceeded {
                actual: limit as usize,
            });
        };

        let last_segment_index = self
            .chain_info
            .max_segment_index()
            .unwrap_or(SegmentIndex::ZERO);

        let mut last_segment_headers = (SegmentIndex::ZERO..=last_segment_index)
            .rev()
            .take(limit as usize)
            .map(|segment_index| self.chain_info.get_segment_header(segment_index))
            .collect::<Vec<_>>();

        last_segment_headers.reverse();

        Ok(last_segment_headers)
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
