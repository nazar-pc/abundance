//! RPC API for the farmer

#![feature(try_blocks)]

use ab_archiving::archiver::NewArchivedSegment;
use ab_client_api::ChainSyncStatus;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::pot::SlotNumber;
use ab_core_primitives::segments::{HistorySize, SegmentHeader, SegmentIndex};
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::Solution;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo,
    MAX_SEGMENT_HEADERS_PER_REQUEST, SlotInfo, SolutionResponse,
};
use ab_networking::libp2p::Multiaddr;
use futures::channel::mpsc;
use futures::{FutureExt, StreamExt, future};
use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
use jsonrpsee::{Extensions, PendingSubscriptionSink};
use parking_lot::Mutex;
use sc_client_api::{AuxStore, BlockBackend};
use sc_consensus_subspace::archiver::{
    ArchivedSegmentNotification, SegmentHeadersStore, recreate_genesis_segment,
};
use sc_consensus_subspace::notification::SubspaceNotificationStream;
use sc_consensus_subspace::slot_worker::{BlockSealingNotification, NewSlotNotification};
use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc::utils::{BoundedVecDeque, PendingSubscription};
use sc_rpc_api::{UnsafeRpcError, check_if_safe};
use sc_utils::mpsc::TracingUnboundedSender;
use schnellru::{ByLength, LruMap};
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus_subspace::{ChainConstants, SubspaceApi};
use sp_runtime::traits::Block as BlockT;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tracing::{debug, error, warn};

const SUBSPACE_ERROR: i32 = 9000;
const BLOCK_SEALING_TIMEOUT: Duration = Duration::from_millis(500);

// TODO: More specific errors instead of `StringError`
/// Top-level error type for the RPC handler.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Errors that can be formatted as a String
    #[error("{0}")]
    StringError(String),
    /// Call to an unsafe RPC was denied.
    #[error(transparent)]
    UnsafeRpcCalled(#[from] UnsafeRpcError),
}

impl From<Error> for ErrorObjectOwned {
    fn from(error: Error) -> Self {
        match error {
            Error::StringError(e) => ErrorObject::owned(SUBSPACE_ERROR + 1, e, None::<()>),
            Error::UnsafeRpcCalled(e) => e.into(),
        }
    }
}

/// Provides rpc methods for interacting with the farmer
#[rpc(server)]
pub trait FarmerRpcApi {
    /// Get metadata necessary for farmer operation
    #[method(name = "getFarmerAppInfo")]
    fn get_farmer_app_info(&self) -> Result<FarmerAppInfo, Error>;

    #[method(name = "submitSolutionResponse", with_extensions)]
    fn submit_solution_response(&self, solution_response: SolutionResponse) -> Result<(), Error>;

    /// Slot info subscription
    #[subscription(
        name = "subscribeSlotInfo" => "slot_info",
        unsubscribe = "unsubscribeSlotInfo",
        item = SlotInfo,
        with_extensions,
    )]
    fn subscribe_slot_info(&self);

    /// Sign block subscription
    #[subscription(
        name = "subscribeBlockSealing" => "block_sealing",
        unsubscribe = "unsubscribeBlockSealing",
        item = BlockSealInfo,
        with_extensions,
    )]
    fn subscribe_block_sealing(&self);

    #[method(name = "submitBlockSeal", with_extensions)]
    fn submit_block_seal(&self, block_seal: BlockSealResponse) -> Result<(), Error>;

    /// Archived segment header subscription
    #[subscription(
        name = "subscribeArchivedSegmentHeader" => "archived_segment_header",
        unsubscribe = "unsubscribeArchivedSegmentHeader",
        item = SegmentHeader,
        with_extensions,
    )]
    fn subscribe_archived_segment_header(&self);

    #[method(name = "segmentHeaders")]
    async fn segment_headers(
        &self,
        segment_indexes: Vec<SegmentIndex>,
    ) -> Result<Vec<Option<SegmentHeader>>, Error>;

    #[method(name = "piece", blocking, with_extensions)]
    fn piece(&self, piece_index: PieceIndex) -> Result<Option<Piece>, Error>;

    #[method(name = "acknowledgeArchivedSegmentHeader", with_extensions)]
    async fn acknowledge_archived_segment_header(
        &self,
        segment_index: SegmentIndex,
    ) -> Result<(), Error>;

    #[method(name = "lastSegmentHeaders")]
    async fn last_segment_headers(&self, limit: u32) -> Result<Vec<Option<SegmentHeader>>, Error>;

    #[method(name = "updateShardMembershipInfo")]
    fn update_shard_membership_info(
        &self,
        info: Vec<FarmerShardMembershipInfo>,
    ) -> Result<(), Error>;
}

#[derive(Default)]
struct ArchivedSegmentHeaderAcknowledgementSenders {
    segment_index: SegmentIndex,
    senders: HashMap<u64, TracingUnboundedSender<()>>,
}

#[derive(Default)]
struct BlockSignatureSenders {
    current_pre_seal_hash: Blake3Hash,
    senders: Vec<async_oneshot::Sender<BlockSealResponse>>,
}

/// In-memory cache of last archived segment, such that when request comes back right after
/// archived segment notification, RPC server is able to answer quickly.
///
/// We store weak reference, such that archived segment is not persisted for longer than
/// necessary occupying RAM.
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

/// Farmer RPC configuration
pub struct FarmerRpcConfig<Client, CSS, AS>
where
    AS: AuxStore + Send + Sync + 'static,
{
    /// Substrate client
    pub client: Arc<Client>,
    /// Task executor that is being used by RPC subscriptions
    pub subscription_executor: SubscriptionTaskExecutor,
    /// New slot notification stream
    pub new_slot_notification_stream: SubspaceNotificationStream<NewSlotNotification>,
    /// Block sealing notification stream
    pub block_sealing_notification_stream: SubspaceNotificationStream<BlockSealingNotification>,
    /// Archived segment notification stream
    pub archived_segment_notification_stream:
        SubspaceNotificationStream<ArchivedSegmentNotification>,
    /// DSN bootstrap nodes
    pub dsn_bootstrap_nodes: Vec<Multiaddr>,
    /// Segment headers store
    pub segment_headers_store: SegmentHeadersStore<AS>,
    /// Chain sync status
    pub chain_sync_status: CSS,
    /// Erasure coding instance
    pub erasure_coding: ErasureCoding,
}

/// Implements the [`FarmerRpcApiServer`] trait for farmer to connect to
pub struct FarmerRpc<Block, Client, CSS, AS>
where
    Block: BlockT,
    CSS: ChainSyncStatus,
{
    client: Arc<Client>,
    subscription_executor: SubscriptionTaskExecutor,
    new_slot_notification_stream: SubspaceNotificationStream<NewSlotNotification>,
    block_sealing_notification_stream: SubspaceNotificationStream<BlockSealingNotification>,
    archived_segment_notification_stream: SubspaceNotificationStream<ArchivedSegmentNotification>,
    solution_response_senders: Arc<Mutex<LruMap<SlotNumber, mpsc::Sender<Solution>>>>,
    block_seal_senders: Arc<Mutex<BlockSignatureSenders>>,
    dsn_bootstrap_nodes: Vec<Multiaddr>,
    segment_headers_store: SegmentHeadersStore<AS>,
    cached_archived_segment: Arc<Mutex<Option<CachedArchivedSegment>>>,
    archived_segment_acknowledgement_senders:
        Arc<Mutex<ArchivedSegmentHeaderAcknowledgementSenders>>,
    next_subscription_id: AtomicU64,
    chain_sync_status: CSS,
    genesis_root: BlockRoot,
    chain_constants: ChainConstants,
    max_pieces_in_sector: u16,
    erasure_coding: ErasureCoding,
    _block: PhantomData<Block>,
}

/// [`FarmerRpc`] is used for notifying subscribers about the arrival of new slots and for
/// submission of solutions (or lack thereof).
///
/// Internally every time slot notifier emits information about a new slot, a notification is sent
/// to every subscriber, after which the RPC server waits for the same number of
/// `submitSolutionResponse` requests with `SolutionResponse` in them or until
/// timeout is exceeded. The first valid solution for a particular slot wins, others are ignored.
impl<Block, Client, CSS, AS> FarmerRpc<Block, Client, CSS, AS>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: SubspaceApi<Block>,
    CSS: ChainSyncStatus,
    AS: AuxStore + Send + Sync + 'static,
{
    /// Creates a new instance of the `FarmerRpc` handler.
    pub fn new(config: FarmerRpcConfig<Client, CSS, AS>) -> Result<Self, ApiError> {
        let info = config.client.info();
        let best_hash = info.best_hash;
        let mut genesis_hash = BlockRoot::default();
        genesis_hash.copy_from_slice(info.genesis_hash.as_ref());
        let runtime_api = config.client.runtime_api();
        let chain_constants = runtime_api.chain_constants(best_hash)?;
        // While the number can technically change in runtime, farmer will not adjust to it on the
        // fly and previous value will remain valid (number only expected to increase), so it is
        // fine to query it only once
        let max_pieces_in_sector = runtime_api.max_pieces_in_sector(best_hash)?;
        let block_authoring_delay = u64::from(chain_constants.block_authoring_delay());
        let block_authoring_delay = usize::try_from(block_authoring_delay)
            .expect("Block authoring delay will never exceed usize on any platform; qed");
        let solution_response_senders_capacity = u32::try_from(block_authoring_delay)
            .expect("Always a tiny constant in the protocol; qed");

        Ok(Self {
            client: config.client,
            subscription_executor: config.subscription_executor,
            new_slot_notification_stream: config.new_slot_notification_stream,
            block_sealing_notification_stream: config.block_sealing_notification_stream,
            archived_segment_notification_stream: config.archived_segment_notification_stream,
            solution_response_senders: Arc::new(Mutex::new(LruMap::new(ByLength::new(
                solution_response_senders_capacity,
            )))),
            block_seal_senders: Arc::default(),
            dsn_bootstrap_nodes: config.dsn_bootstrap_nodes,
            segment_headers_store: config.segment_headers_store,
            cached_archived_segment: Arc::default(),
            archived_segment_acknowledgement_senders: Arc::default(),
            next_subscription_id: AtomicU64::default(),
            chain_sync_status: config.chain_sync_status,
            genesis_root: genesis_hash,
            chain_constants,
            max_pieces_in_sector,
            erasure_coding: config.erasure_coding,
            _block: PhantomData,
        })
    }
}

#[async_trait]
impl<Block, Client, CSS, AS> FarmerRpcApiServer for FarmerRpc<Block, Client, CSS, AS>
where
    Block: BlockT,
    Client: HeaderBackend<Block> + BlockBackend<Block> + Send + Sync + 'static,
    CSS: ChainSyncStatus,
    AS: AuxStore + Send + Sync + 'static,
{
    fn get_farmer_app_info(&self) -> Result<FarmerAppInfo, Error> {
        let last_segment_index = self
            .segment_headers_store
            .max_segment_index()
            .unwrap_or(SegmentIndex::ZERO);

        let farmer_app_info: Result<FarmerAppInfo, ApiError> = try {
            let chain_constants = &self.chain_constants;
            let protocol_info = FarmerProtocolInfo {
                history_size: HistorySize::from(last_segment_index),
                max_pieces_in_sector: self.max_pieces_in_sector,
                recent_segments: chain_constants.recent_segments(),
                recent_history_fraction: chain_constants.recent_history_fraction(),
                min_sector_lifetime: chain_constants.min_sector_lifetime(),
            };

            FarmerAppInfo {
                genesis_root: self.genesis_root,
                dsn_bootstrap_nodes: self.dsn_bootstrap_nodes.clone(),
                syncing: self.chain_sync_status.is_syncing(),
                farming_timeout: chain_constants
                    .slot_duration()
                    .as_duration()
                    .mul_f64(chain_constants.block_authoring_delay().as_u64() as f64),
                protocol_info,
            }
        };

        farmer_app_info.map_err(|error| {
            error!("Failed to get data from runtime API: {}", error);
            Error::StringError("Internal error".to_string())
        })
    }

    fn submit_solution_response(
        &self,
        ext: &Extensions,
        solution_response: SolutionResponse,
    ) -> Result<(), Error> {
        check_if_safe(ext)?;

        let slot = solution_response.slot_number;
        let mut solution_response_senders = self.solution_response_senders.lock();

        let success = solution_response_senders
            .peek_mut(&slot)
            .and_then(|sender| sender.try_send(solution_response.solution).ok())
            .is_some();

        if !success {
            warn!(
                %slot,
                "Solution was ignored, likely because farmer was too slow"
            );

            return Err(Error::StringError("Solution was ignored".to_string()));
        }

        Ok(())
    }

    fn subscribe_slot_info(&self, pending: PendingSubscriptionSink, ext: &Extensions) {
        let executor = self.subscription_executor.clone();
        let solution_response_senders = self.solution_response_senders.clone();
        let allow_solutions = check_if_safe(ext).is_ok();

        let handle_slot_notification = move |new_slot_notification| {
            let NewSlotNotification {
                new_slot_info,
                mut solution_sender,
            } = new_slot_notification;

            let slot_number = new_slot_info.slot;

            // Only handle solution responses in case unsafe APIs are allowed
            if allow_solutions {
                // Store solution sender so that we can retrieve it when solution comes from
                // the farmer
                let mut solution_response_senders = solution_response_senders.lock();
                if solution_response_senders.peek(&slot_number).is_none() {
                    let (response_sender, mut response_receiver) = mpsc::channel(1);

                    solution_response_senders.insert(slot_number, response_sender);

                    // Wait for solutions and transform proposed proof of space solutions
                    // into data structure `sc-consensus-subspace` expects
                    let forward_solution_fut = async move {
                        while let Some(solution) = response_receiver.next().await {
                            let public_key_hash = solution.public_key_hash;
                            let sector_index = solution.sector_index;

                            if solution_sender.try_send(solution).is_err() {
                                warn!(
                                    slot = %slot_number,
                                    %sector_index,
                                    %public_key_hash,
                                    "Solution receiver is closed, likely because farmer was too slow"
                                );
                            }
                        }
                    };

                    executor.spawn(
                        "slot-info-forward",
                        Some("rpc"),
                        Box::pin(forward_solution_fut),
                    );
                }
            }

            let global_challenge = new_slot_info
                .proof_of_time
                .derive_global_challenge(slot_number);

            // This will be sent to the farmer
            SlotInfo {
                slot: slot_number,
                global_challenge,
                solution_range: new_slot_info.solution_range,
                entropy: Default::default(),
                num_shards: NumShards {
                    intermediate_shards: 0,
                    leaf_shards_per_intermediate_shard: 0,
                },
            }
        };
        let stream = self
            .new_slot_notification_stream
            .subscribe()
            .map(handle_slot_notification);

        self.subscription_executor.spawn(
            "slot-info-subscription",
            Some("rpc"),
            PendingSubscription::from(pending)
                .pipe_from_stream(stream, BoundedVecDeque::default())
                .boxed(),
        );
    }

    fn subscribe_block_sealing(&self, pending: PendingSubscriptionSink, ext: &Extensions) {
        if check_if_safe(ext).is_err() {
            debug!("Unsafe subscribe_block_sealing ignored");
            return;
        }

        let executor = self.subscription_executor.clone();
        let block_seal_senders = self.block_seal_senders.clone();

        let stream = self.block_sealing_notification_stream.subscribe().map(
            move |block_sealing_notification| {
                let BlockSealingNotification {
                    pre_seal_hash,
                    public_key_hash,
                    signature_sender,
                } = block_sealing_notification;

                let (response_sender, response_receiver) = async_oneshot::oneshot();

                // Store signature sender so that we can retrieve it when solution comes from
                // the farmer
                {
                    let mut block_seal_senders = block_seal_senders.lock();

                    if block_seal_senders.current_pre_seal_hash != pre_seal_hash {
                        block_seal_senders.current_pre_seal_hash = pre_seal_hash;
                        block_seal_senders.senders.clear();
                    }

                    block_seal_senders.senders.push(response_sender);
                }

                // Wait for solutions and transform proposed proof of space solutions into
                // data structure `sc-consensus-subspace` expects
                let forward_signature_fut = async move {
                    if let Ok(block_seal) = response_receiver.await {
                        let _ = signature_sender.unbounded_send(block_seal.seal);
                    }
                };

                // Run above future with timeout
                executor.spawn(
                    "block-signing-forward",
                    Some("rpc"),
                    future::select(
                        futures_timer::Delay::new(BLOCK_SEALING_TIMEOUT),
                        Box::pin(forward_signature_fut),
                    )
                    .map(|_| ())
                    .boxed(),
                );

                // This will be sent to the farmer
                BlockSealInfo {
                    pre_seal_hash,
                    public_key_hash,
                }
            },
        );

        self.subscription_executor.spawn(
            "block-signing-subscription",
            Some("rpc"),
            PendingSubscription::from(pending)
                .pipe_from_stream(stream, BoundedVecDeque::default())
                .boxed(),
        );
    }

    fn submit_block_seal(
        &self,
        ext: &Extensions,
        block_seal: BlockSealResponse,
    ) -> Result<(), Error> {
        check_if_safe(ext)?;

        let block_seal_senders = self.block_seal_senders.clone();

        // TODO: This doesn't track what client sent a solution, allowing some clients to send
        //  multiple (https://github.com/paritytech/jsonrpsee/issues/452)
        let mut block_seal_senders = block_seal_senders.lock();

        if block_seal_senders.current_pre_seal_hash == block_seal.pre_seal_hash
            && let Some(mut sender) = block_seal_senders.senders.pop()
        {
            let _ = sender.send(block_seal);
        }

        Ok(())
    }

    fn subscribe_archived_segment_header(
        &self,
        pending: PendingSubscriptionSink,
        ext: &Extensions,
    ) {
        let archived_segment_acknowledgement_senders =
            self.archived_segment_acknowledgement_senders.clone();

        let cached_archived_segment = Arc::clone(&self.cached_archived_segment);
        let subscription_id = self.next_subscription_id.fetch_add(1, Ordering::Relaxed);
        let allow_acknowledgements = check_if_safe(ext).is_ok();

        let stream = self
            .archived_segment_notification_stream
            .subscribe()
            .filter_map(move |archived_segment_notification| {
                let ArchivedSegmentNotification {
                    archived_segment,
                    acknowledgement_sender,
                } = archived_segment_notification;

                let segment_index = archived_segment.segment_header.segment_index();

                // Store acknowledgment sender so that we can retrieve it when acknowledgement
                // comes from the farmer, but only if unsafe APIs are allowed
                let maybe_archived_segment_header = if allow_acknowledgements {
                    let mut archived_segment_acknowledgement_senders =
                        archived_segment_acknowledgement_senders.lock();

                    if archived_segment_acknowledgement_senders.segment_index != segment_index {
                        archived_segment_acknowledgement_senders.segment_index = segment_index;
                        archived_segment_acknowledgement_senders.senders.clear();
                    }

                    let maybe_archived_segment_header =
                        match archived_segment_acknowledgement_senders
                            .senders
                            .entry(subscription_id)
                        {
                            Entry::Occupied(_) => {
                                // No need to do anything, farmer is processing request
                                None
                            }
                            Entry::Vacant(entry) => {
                                entry.insert(acknowledgement_sender);

                                // This will be sent to the farmer
                                Some(archived_segment.segment_header)
                            }
                        };

                    cached_archived_segment
                        .lock()
                        .replace(CachedArchivedSegment::Weak(Arc::downgrade(
                            &archived_segment,
                        )));

                    maybe_archived_segment_header
                } else {
                    // In case unsafe APIs are not allowed, just return segment header without
                    // requiring it to be acknowledged
                    Some(archived_segment.segment_header)
                };

                Box::pin(async move { maybe_archived_segment_header })
            });

        let archived_segment_acknowledgement_senders =
            self.archived_segment_acknowledgement_senders.clone();
        let fut = async move {
            PendingSubscription::from(pending)
                .pipe_from_stream(stream, BoundedVecDeque::default())
                .await;

            let mut archived_segment_acknowledgement_senders =
                archived_segment_acknowledgement_senders.lock();

            archived_segment_acknowledgement_senders
                .senders
                .remove(&subscription_id);
        };

        self.subscription_executor.spawn(
            "archived-segment-header-subscription",
            Some("rpc"),
            fut.boxed(),
        );
    }

    async fn acknowledge_archived_segment_header(
        &self,
        ext: &Extensions,
        segment_index: SegmentIndex,
    ) -> Result<(), Error> {
        check_if_safe(ext)?;

        let archived_segment_acknowledgement_senders =
            self.archived_segment_acknowledgement_senders.clone();

        let maybe_sender = {
            let mut archived_segment_acknowledgement_senders_guard =
                archived_segment_acknowledgement_senders.lock();

            (archived_segment_acknowledgement_senders_guard.segment_index == segment_index)
                .then(|| {
                    let last_key = *archived_segment_acknowledgement_senders_guard
                        .senders
                        .keys()
                        .next()?;

                    archived_segment_acknowledgement_senders_guard
                        .senders
                        .remove(&last_key)
                })
                .flatten()
        };

        if let Some(sender) = maybe_sender
            && let Err(error) = sender.unbounded_send(())
            && !error.is_closed()
        {
            warn!("Failed to acknowledge archived segment: {error}");
        }

        debug!(%segment_index, "Acknowledged archived segment.");

        Ok(())
    }

    // Note: this RPC uses the cached archived segment, which is only updated by archived segments subscriptions
    fn piece(
        &self,
        ext: &Extensions,
        requested_piece_index: PieceIndex,
    ) -> Result<Option<Piece>, Error> {
        check_if_safe(ext)?;

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

                    debug!(%requested_piece_index, "Re-creating genesis segment on demand");

                    // Try to re-create genesis segment on demand
                    match recreate_genesis_segment(&*self.client, self.erasure_coding.clone()) {
                        Ok(Some(archived_segment)) => {
                            let archived_segment = Arc::new(archived_segment);
                            cached_archived_segment.replace(CachedArchivedSegment::Genesis(
                                Arc::clone(&archived_segment),
                            ));
                            archived_segment
                        }
                        Ok(None) => {
                            return Ok(None);
                        }
                        Err(error) => {
                            error!(%error, "Failed to re-create genesis segment");

                            return Err(Error::StringError(
                                "Failed to re-create genesis segment".to_string(),
                            ));
                        }
                    }
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
        segment_indexes: Vec<SegmentIndex>,
    ) -> Result<Vec<Option<SegmentHeader>>, Error> {
        if segment_indexes.len() > MAX_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "segment_indexes length exceed the limit: {} ",
                segment_indexes.len()
            );

            return Err(Error::StringError(format!(
                "segment_indexes length exceed the limit {MAX_SEGMENT_HEADERS_PER_REQUEST}"
            )));
        };

        Ok(segment_indexes
            .into_iter()
            .map(|segment_index| self.segment_headers_store.get_segment_header(segment_index))
            .collect())
    }

    async fn last_segment_headers(&self, limit: u32) -> Result<Vec<Option<SegmentHeader>>, Error> {
        if limit as usize > MAX_SEGMENT_HEADERS_PER_REQUEST {
            error!(
                "Request limit ({}) exceed the server limit: {} ",
                limit, MAX_SEGMENT_HEADERS_PER_REQUEST
            );

            return Err(Error::StringError(format!(
                "Request limit ({limit}) exceed the server limit: {MAX_SEGMENT_HEADERS_PER_REQUEST}"
            )));
        };

        let last_segment_index = self
            .segment_headers_store
            .max_segment_index()
            .unwrap_or(SegmentIndex::ZERO);

        let mut last_segment_headers = (SegmentIndex::ZERO..=last_segment_index)
            .rev()
            .take(limit as usize)
            .map(|segment_index| self.segment_headers_store.get_segment_header(segment_index))
            .collect::<Vec<_>>();

        last_segment_headers.reverse();

        Ok(last_segment_headers)
    }

    fn update_shard_membership_info(
        &self,
        _info: Vec<FarmerShardMembershipInfo>,
    ) -> Result<(), Error> {
        Ok(())
    }
}
