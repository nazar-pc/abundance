//! Slot worker drives block and vote production based on slots produced in [`sc_proof_of_time`].
//!
//! While slot worker uses [`sc_consensus_slots`], it is not driven by time, but instead by Proof of
//! Time that is produced by [`PotSourceWorker`](sc_proof_of_time::source::PotSourceWorker).
//!
//! Each time a new proof is found, [`PotSlotWorker::on_proof`] is called and corresponding
//! [`SlotInfo`] notification is sent ([`SubspaceLink::new_slot_notification_stream`]) to farmers to
//! do the audit and try to prove they have a solution without actually waiting for the response.
//! [`ChainConstants::block_authoring_delay`](sp_consensus_subspace::ChainConstants::block_authoring_delay)
//! slots later (when corresponding future proof arrives) all the solutions produced by farmers so
//! far are collected and corresponding block and/or votes are produced. In case PoT chain reorg
//! happens, outdated solutions (they are tied to proofs of time) are thrown away.
//!
//! Custom [`SubspaceSyncOracle`] wrapper is introduced due to Subspace-specific changes comparing
//! to the base Substrate behavior where major syncing is assumed to not happen in case authoring is
//! forced.

use crate::SubspaceLink;
use crate::archiver::SegmentHeadersStore;
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, SlotNumber};
use ab_core_primitives::sectors::SectorId;
use ab_core_primitives::solutions::{
    Solution, SolutionRange, SolutionVerifyError, SolutionVerifyParams,
    SolutionVerifyPieceCheckParams,
};
use ab_proof_of_space::Table;
use futures::channel::mpsc;
use futures::{StreamExt, TryFutureExt};
use sc_client_api::AuxStore;
use sc_consensus::block_import::{BlockImportParams, StateAction};
use sc_consensus::{BoxBlockImport, JustificationSyncLink, StorageChanges};
use sc_consensus_slots::{
    BackoffAuthoringBlocksStrategy, SimpleSlotWorker, SlotInfo, SlotLenienceType, SlotProportion,
};
use sc_proof_of_time::PotSlotWorker;
use sc_proof_of_time::verifier::PotVerifier;
use sc_telemetry::TelemetryHandle;
use sc_utils::mpsc::{TracingUnboundedSender, tracing_unbounded};
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::{Error as ClientError, HeaderBackend, HeaderMetadata};
use sp_consensus::{BlockOrigin, Environment, Error as ConsensusError, Proposer, SyncOracle};
use sp_consensus_slots::Slot;
use sp_consensus_subspace::digests::{
    CompatibleDigestItem, PreDigest, PreDigestPotInfo, extract_pre_digest,
};
use sp_consensus_subspace::{PotNextSlotInput, SubspaceApi, SubspaceJustification};
use sp_runtime::traits::{Block as BlockT, Header, NumberFor, Zero};
use sp_runtime::{DigestItem, Justification, Justifications};
use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use subspace_verification::ed25519::RewardSignature;
use subspace_verification::is_reward_signature_valid;
use tracing::{debug, error, info, warn};

/// Large enough size for any practical purposes, there shouldn't be even this many solutions.
const PENDING_SOLUTIONS_CHANNEL_CAPACITY: usize = 10;

/// Subspace sync oracle.
///
/// Subspace sync oracle that takes into account force authoring flag, allowing to bootstrap
/// Subspace network from scratch due to our fork of Substrate where sync state of nodes depends on
/// connected nodes (none of which will be synced initially). It also accounts for DSN sync, when
/// normal Substrate sync is paused, which might happen before Substrate's internals decide there is
/// a sync happening, but DSN sync is already in progress.
#[derive(Debug, Clone)]
pub struct SubspaceSyncOracle<SO>
where
    SO: SyncOracle + Send + Sync,
{
    force_authoring: bool,
    pause_sync: Arc<AtomicBool>,
    inner: SO,
}

impl<SO> SyncOracle for SubspaceSyncOracle<SO>
where
    SO: SyncOracle + Send + Sync,
{
    fn is_major_syncing(&self) -> bool {
        // This allows slot worker to produce blocks even when it is offline, which according to
        // modified Substrate fork will happen when node is offline or connected to non-synced peers
        // (default state), it also accounts for DSN sync
        (!self.force_authoring && self.inner.is_major_syncing())
            || self.pause_sync.load(Ordering::Acquire)
    }

    fn is_offline(&self) -> bool {
        self.inner.is_offline()
    }
}

impl<SO> SubspaceSyncOracle<SO>
where
    SO: SyncOracle + Send + Sync,
{
    /// Create new instance
    pub fn new(
        force_authoring: bool,
        pause_sync: Arc<AtomicBool>,
        substrate_sync_oracle: SO,
    ) -> Self {
        Self {
            force_authoring,
            pause_sync,
            inner: substrate_sync_oracle,
        }
    }
}

/// Information about new slot that just arrived
#[derive(Debug, Copy, Clone)]
pub struct NewSlotInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// The PoT output for `slot`
    pub proof_of_time: PotOutput,
    /// Acceptable solution range for block authoring
    pub solution_range: SolutionRange,
}

/// New slot notification with slot information and sender for solution for the slot.
#[derive(Debug, Clone)]
pub struct NewSlotNotification {
    /// New slot information.
    pub new_slot_info: NewSlotInfo,
    /// Sender that can be used to send solutions for the slot.
    pub solution_sender: mpsc::Sender<Solution>,
}
/// Notification with a hash that needs to be signed to receive reward and sender for signature.
#[derive(Debug, Clone)]
pub struct RewardSigningNotification {
    /// Hash to be signed.
    pub hash: Blake3Hash,
    /// Public key hash of the plot identity that should create signature.
    pub public_key_hash: Blake3Hash,
    /// Sender that can be used to send signature for the header.
    pub signature_sender: TracingUnboundedSender<RewardSignature>,
}

/// Parameters for [`SubspaceSlotWorker`]
pub struct SubspaceSlotWorkerOptions<Block, Client, E, SO, L, BS, AS>
where
    Block: BlockT,
    SO: SyncOracle + Send + Sync,
{
    /// The client to use
    pub client: Arc<Client>,
    /// The environment we are producing blocks for.
    pub env: E,
    /// The underlying block-import object to supply our produced blocks to.
    /// This must be a `SubspaceBlockImport` or a wrapper of it, otherwise
    /// critical consensus logic will be omitted.
    pub block_import: BoxBlockImport<Block>,
    /// A sync oracle
    pub sync_oracle: SubspaceSyncOracle<SO>,
    /// Hook into the sync module to control the justification sync process.
    pub justification_sync_link: L,
    /// Force authoring of blocks even if we are offline
    pub force_authoring: bool,
    /// Strategy and parameters for backing off block production.
    pub backoff_authoring_blocks: Option<BS>,
    /// The source of timestamps for relative slots
    pub subspace_link: SubspaceLink,
    /// Persistent storage of segment headers
    pub segment_headers_store: SegmentHeadersStore<AS>,
    /// The proportion of the slot dedicated to proposing.
    ///
    /// The block proposing will be limited to this proportion of the slot from the starting of the
    /// slot. However, the proposing can still take longer when there is some lenience factor applied,
    /// because there were no blocks produced for some slots.
    pub block_proposal_slot_portion: SlotProportion,
    /// The maximum proportion of the slot dedicated to proposing with any lenience factor applied
    /// due to no blocks being produced.
    pub max_block_proposal_slot_portion: Option<SlotProportion>,
    /// Proof of time verifier
    pub pot_verifier: PotVerifier,
}

/// Subspace slot worker responsible for block and vote production
pub struct SubspaceSlotWorker<PosTable, Block, Client, E, SO, L, BS, AS>
where
    Block: BlockT,
    SO: SyncOracle + Send + Sync,
{
    client: Arc<Client>,
    block_import: BoxBlockImport<Block>,
    env: E,
    sync_oracle: SubspaceSyncOracle<SO>,
    justification_sync_link: L,
    force_authoring: bool,
    backoff_authoring_blocks: Option<BS>,
    subspace_link: SubspaceLink,
    block_proposal_slot_portion: SlotProportion,
    max_block_proposal_slot_portion: Option<SlotProportion>,
    segment_headers_store: SegmentHeadersStore<AS>,
    /// Solution receivers for challenges that were sent to farmers and expected to be received
    /// eventually
    pending_solutions: BTreeMap<SlotNumber, mpsc::Receiver<Solution>>,
    /// Collection of PoT slots that can be retrieved later if needed by block production
    pot_checkpoints: BTreeMap<SlotNumber, PotCheckpoints>,
    pot_verifier: PotVerifier,
    _pos_table: PhantomData<PosTable>,
}

impl<PosTable, Block, Client, E, SO, L, BS, AS> PotSlotWorker<Block>
    for SubspaceSlotWorker<PosTable, Block, Client, E, SO, L, BS, AS>
where
    Block: BlockT,
    Client: HeaderBackend<Block> + ProvideRuntimeApi<Block>,
    Client::Api: SubspaceApi<Block>,
    SO: SyncOracle + Send + Sync,
{
    fn on_proof(&mut self, slot: SlotNumber, checkpoints: PotCheckpoints) {
        // Remove checkpoints from future slots, if present they are out of date anyway
        self.pot_checkpoints
            .retain(|&stored_slot, _checkpoints| stored_slot < slot);

        self.pot_checkpoints.insert(slot, checkpoints);

        if self.sync_oracle.is_major_syncing() {
            debug!("Skipping farming slot {slot} due to sync");
            return;
        }

        let maybe_root_plot_public_key_hash = self
            .client
            .runtime_api()
            .root_plot_public_key_hash(self.client.info().best_hash)
            .ok()
            .flatten();
        if maybe_root_plot_public_key_hash.is_some() && !self.force_authoring {
            debug!(
                "Skipping farming slot {slot} due to root public key present and force authoring \
                not enabled"
            );
            return;
        }

        let proof_of_time = checkpoints.output();

        // NOTE: Best hash is not necessarily going to be the parent of corresponding block, but
        // solution range shouldn't be too far off
        let best_hash = self.client.info().best_hash;
        let solution_range = match extract_solution_range_for_block(self.client.as_ref(), best_hash)
        {
            Ok(solution_range) => solution_range,
            Err(error) => {
                warn!(
                    %slot,
                    %best_hash,
                    %error,
                    "Failed to extract solution ranges for block"
                );
                return;
            }
        };

        let new_slot_info = NewSlotInfo {
            slot,
            proof_of_time,
            solution_range,
        };
        let (solution_sender, solution_receiver) =
            mpsc::channel(PENDING_SOLUTIONS_CHANNEL_CAPACITY);

        self.subspace_link
            .new_slot_notification_sender
            .notify(|| NewSlotNotification {
                new_slot_info,
                solution_sender,
            });

        self.pending_solutions.insert(slot, solution_receiver);
    }
}

#[async_trait::async_trait]
impl<PosTable, Block, Client, E, Error, SO, L, BS, AS> SimpleSlotWorker<Block>
    for SubspaceSlotWorker<PosTable, Block, Client, E, SO, L, BS, AS>
where
    PosTable: Table,
    Block: BlockT,
    Client: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + HeaderMetadata<Block, Error = ClientError>
        + AuxStore
        + 'static,
    Client::Api: SubspaceApi<Block>,
    E: Environment<Block, Error = Error> + Send + Sync,
    E::Proposer: Proposer<Block, Error = Error>,
    SO: SyncOracle + Send + Sync,
    L: JustificationSyncLink<Block>,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<Block>> + Send + Sync,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    AS: AuxStore + Send + Sync + 'static,
    BlockNumber: From<<Block::Header as Header>::Number>,
{
    type BlockImport = BoxBlockImport<Block>;
    type SyncOracle = SubspaceSyncOracle<SO>;
    type JustificationSyncLink = L;
    type CreateProposer =
        Pin<Box<dyn Future<Output = Result<E::Proposer, ConsensusError>> + Send + 'static>>;
    type Proposer = E::Proposer;
    type Claim = (PreDigest, SubspaceJustification);
    type AuxData = ();

    fn logging_target(&self) -> &'static str {
        "subspace"
    }

    fn block_import(&mut self) -> &mut Self::BlockImport {
        &mut self.block_import
    }

    fn aux_data(
        &self,
        _parent: &Block::Header,
        _slot: Slot,
    ) -> Result<Self::AuxData, ConsensusError> {
        Ok(())
    }

    fn authorities_len(&self, _epoch_data: &Self::AuxData) -> Option<usize> {
        // This function is used in `sc-consensus-slots` in order to determine whether it is
        // possible to skip block production under certain circumstances, returning `None` or any
        // number smaller or equal to `1` disables that functionality and we don't want that.
        Some(2)
    }

    async fn claim_slot(
        &mut self,
        parent_header: &Block::Header,
        slot: Slot,
        _aux_data: &Self::AuxData,
    ) -> Option<Self::Claim> {
        let slot = SlotNumber::new(<u64 as From<Slot>>::from(slot));

        let parent_pre_digest = match extract_pre_digest(parent_header) {
            Ok(pre_digest) => pre_digest,
            Err(error) => {
                error!(
                    %error,
                    "Failed to parse pre-digest out of parent header"
                );

                return None;
            }
        };
        let parent_slot = parent_pre_digest.slot;

        if slot <= parent_slot {
            debug!(
                "Skipping claiming slot {slot} it must be higher than parent slot {parent_slot}",
            );

            return None;
        } else {
            debug!(%slot, "Attempting to claim slot");
        }

        let chain_constants = self.subspace_link.chain_constants();

        let parent_hash = parent_header.hash();
        let runtime_api = self.client.runtime_api();

        let solution_range =
            extract_solution_range_for_block(self.client.as_ref(), parent_hash).ok()?;

        let maybe_root_plot_public_key_hash =
            runtime_api.root_plot_public_key_hash(parent_hash).ok()?;

        let parent_pot_parameters = runtime_api.pot_parameters(parent_hash).ok()?;
        let parent_future_slot = if parent_header.number().is_zero() {
            parent_slot
        } else {
            parent_slot + chain_constants.block_authoring_delay()
        };

        let (proof_of_time, future_proof_of_time, pot_justification) = {
            // Remove checkpoints from old slots we will not need anymore
            self.pot_checkpoints
                .retain(|&stored_slot, _checkpoints| stored_slot > parent_slot);

            let proof_of_time = self.pot_checkpoints.get(&slot)?.output();

            // Future slot for which proof must be available before authoring block at this slot
            let future_slot = slot + chain_constants.block_authoring_delay();

            let pot_input = if parent_header.number().is_zero() {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: parent_pot_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    parent_pot_parameters.slot_iterations,
                    parent_slot,
                    parent_pre_digest.pot_info.proof_of_time,
                    &parent_pot_parameters.next_change,
                )
            };

            // Ensure proof of time is valid according to parent block
            if !self.pot_verifier.is_output_valid(
                pot_input,
                slot - parent_slot,
                proof_of_time,
                parent_pot_parameters.next_change,
            ) {
                warn!(
                    %slot,
                    ?pot_input,
                    ?parent_pot_parameters,
                    "Proof of time is invalid, skipping block authoring at slot"
                );
                return None;
            }

            let mut checkpoints_pot_input = if parent_header.number().is_zero() {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: parent_pot_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    parent_pot_parameters.slot_iterations,
                    parent_future_slot,
                    parent_pre_digest.pot_info.future_proof_of_time,
                    &parent_pot_parameters.next_change,
                )
            };
            let seed = checkpoints_pot_input.seed;

            let mut checkpoints =
                Vec::with_capacity((future_slot - parent_future_slot).as_u64() as usize);

            for slot in parent_future_slot + SlotNumber::ONE..=future_slot {
                let maybe_slot_checkpoints = self.pot_verifier.get_checkpoints(
                    checkpoints_pot_input.slot_iterations,
                    checkpoints_pot_input.seed,
                );
                let Some(slot_checkpoints) = maybe_slot_checkpoints else {
                    warn!("Proving failed during block authoring");
                    return None;
                };

                checkpoints.push(slot_checkpoints);

                checkpoints_pot_input = PotNextSlotInput::derive(
                    checkpoints_pot_input.slot_iterations,
                    slot,
                    slot_checkpoints.output(),
                    &parent_pot_parameters.next_change,
                );
            }

            let future_proof_of_time = checkpoints
                .last()
                .expect("Never empty, there is at least one slot between blocks; qed")
                .output();

            let pot_justification = SubspaceJustification::PotCheckpoints { seed, checkpoints };

            (proof_of_time, future_proof_of_time, pot_justification)
        };

        let mut solution_receiver = {
            // Remove receivers for old slots we will not need anymore
            self.pending_solutions
                .retain(|&stored_slot, _solution_receiver| stored_slot >= slot);

            let mut solution_receiver = self.pending_solutions.remove(&slot)?;
            // Time is out, we will not accept any more solutions
            solution_receiver.close();
            solution_receiver
        };

        let mut maybe_pre_digest = None;

        while let Some(solution) = solution_receiver.next().await {
            if let Some(root_plot_public_key_hash) = &maybe_root_plot_public_key_hash
                && &solution.public_key_hash != root_plot_public_key_hash
            {
                // Only root plot public key is allowed, no need to even try to claim block or
                // vote.
                continue;
            }

            let sector_id = SectorId::new(
                &solution.public_key_hash,
                solution.sector_index,
                solution.history_size,
            );

            let history_size = runtime_api.history_size(parent_hash).ok()?;
            let max_pieces_in_sector = runtime_api.max_pieces_in_sector(parent_hash).ok()?;

            let segment_index = sector_id
                .derive_piece_index(
                    solution.piece_offset,
                    solution.history_size,
                    max_pieces_in_sector,
                    chain_constants.recent_segments(),
                    chain_constants.recent_history_fraction(),
                )
                .segment_index();
            let maybe_segment_root = self
                .segment_headers_store
                .get_segment_header(segment_index)
                .map(|segment_header| segment_header.segment_root);

            let segment_root = match maybe_segment_root {
                Some(segment_root) => segment_root,
                None => {
                    warn!(
                        %slot,
                        %segment_index,
                        "Segment root not found",
                    );
                    continue;
                }
            };
            let sector_expiration_check_segment_index = match solution
                .history_size
                .sector_expiration_check(chain_constants.min_sector_lifetime())
            {
                Some(sector_expiration_check) => sector_expiration_check.segment_index(),
                None => {
                    continue;
                }
            };
            let sector_expiration_check_segment_root = runtime_api
                .segment_root(parent_hash, sector_expiration_check_segment_index)
                .ok()?;

            let solution_verification_result = solution.verify::<PosTable>(
                slot,
                &SolutionVerifyParams {
                    proof_of_time,
                    solution_range,
                    piece_check_params: Some(SolutionVerifyPieceCheckParams {
                        max_pieces_in_sector,
                        segment_root,
                        recent_segments: chain_constants.recent_segments(),
                        recent_history_fraction: chain_constants.recent_history_fraction(),
                        min_sector_lifetime: chain_constants.min_sector_lifetime(),
                        current_history_size: history_size,
                        sector_expiration_check_segment_root,
                    }),
                },
            );

            match solution_verification_result {
                Ok(()) => {
                    if maybe_pre_digest.is_none() {
                        info!(%slot, "🚜 Claimed block at slot");
                        maybe_pre_digest.replace(PreDigest {
                            slot,
                            solution,
                            pot_info: PreDigestPotInfo {
                                proof_of_time,
                                future_proof_of_time,
                            },
                        });
                    } else {
                        info!(
                            %slot,
                            "Skipping solution that has quality sufficient for block because \
                            block pre-digest was already created",
                        );
                    }
                }
                Err(error @ SolutionVerifyError::OutsideSolutionRange { .. }) => {
                    // Solution range might have just adjusted, but when farmer was auditing they
                    // didn't know about this, so downgrade warning to debug message
                    if runtime_api
                        .solution_ranges(parent_hash)
                        .ok()
                        .and_then(|solution_ranges| solution_ranges.next)
                        .is_some()
                    {
                        debug!(
                            %slot,
                            %error,
                            "Invalid solution received",
                        );
                    } else {
                        warn!(
                            %slot,
                            %error,
                            "Invalid solution received",
                        );
                    }
                }
                Err(error) => {
                    warn!(
                        %slot,
                        %error,
                        "Invalid solution received",
                    );
                }
            }
        }

        maybe_pre_digest.map(|pre_digest| (pre_digest, pot_justification))
    }

    fn pre_digest_data(
        &self,
        _slot: Slot,
        (pre_digest, _justification): &Self::Claim,
    ) -> Vec<DigestItem> {
        vec![DigestItem::subspace_pre_digest(pre_digest)]
    }

    async fn block_import_params(
        &self,
        header: Block::Header,
        header_hash: &Block::Hash,
        body: Vec<Block::Extrinsic>,
        storage_changes: sc_consensus_slots::StorageChanges<Block>,
        (pre_digest, justification): Self::Claim,
        _aux_data: Self::AuxData,
    ) -> Result<BlockImportParams<Block>, ConsensusError> {
        let signature = self
            .sign_reward(
                Blake3Hash::new(
                    header_hash
                        .as_ref()
                        .try_into()
                        .expect("Block hash is exactly 32 bytes; qed"),
                ),
                pre_digest.solution.public_key_hash,
            )
            .await?;

        let digest_item = DigestItem::subspace_seal(signature);

        let mut import_block = BlockImportParams::new(BlockOrigin::Own, header);
        import_block.post_digests.push(digest_item);
        import_block.body = Some(body);
        import_block.state_action =
            StateAction::ApplyChanges(StorageChanges::Changes(storage_changes));
        import_block
            .justifications
            .replace(Justifications::from(Justification::from(justification)));

        Ok(import_block)
    }

    fn force_authoring(&self) -> bool {
        self.force_authoring
    }

    fn should_backoff(&self, slot: Slot, chain_head: &Block::Header) -> bool {
        if let Some(strategy) = &self.backoff_authoring_blocks
            && let Ok(chain_head_slot) = extract_pre_digest(chain_head).map(|digest| digest.slot)
        {
            return strategy.should_backoff(
                *chain_head.number(),
                Slot::from(chain_head_slot.as_u64()),
                self.client.info().finalized_number,
                slot,
                self.logging_target(),
            );
        }
        false
    }

    fn sync_oracle(&mut self) -> &mut Self::SyncOracle {
        &mut self.sync_oracle
    }

    fn justification_sync_link(&mut self) -> &mut Self::JustificationSyncLink {
        &mut self.justification_sync_link
    }

    fn proposer(&mut self, block: &Block::Header) -> Self::CreateProposer {
        Box::pin(
            self.env
                .init(block)
                .map_err(|e| ConsensusError::ClientImport(e.to_string())),
        )
    }

    fn telemetry(&self) -> Option<TelemetryHandle> {
        None
    }

    fn proposing_remaining_duration(&self, slot_info: &SlotInfo<Block>) -> std::time::Duration {
        let parent_slot = extract_pre_digest(&slot_info.chain_head)
            .ok()
            .map(|d| d.slot);

        sc_consensus_slots::proposing_remaining_duration(
            parent_slot.map(|parent_slot| Slot::from(parent_slot.as_u64())),
            slot_info,
            &self.block_proposal_slot_portion,
            self.max_block_proposal_slot_portion.as_ref(),
            SlotLenienceType::Exponential,
            self.logging_target(),
        )
    }
}

impl<PosTable, Block, Client, E, Error, SO, L, BS, AS>
    SubspaceSlotWorker<PosTable, Block, Client, E, SO, L, BS, AS>
where
    PosTable: Table,
    Block: BlockT,
    Client: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + HeaderMetadata<Block, Error = ClientError>
        + AuxStore
        + 'static,
    Client::Api: SubspaceApi<Block>,
    E: Environment<Block, Error = Error> + Send + Sync,
    E::Proposer: Proposer<Block, Error = Error>,
    SO: SyncOracle + Send + Sync,
    L: JustificationSyncLink<Block>,
    BS: BackoffAuthoringBlocksStrategy<NumberFor<Block>> + Send + Sync,
    Error: std::error::Error + Send + From<ConsensusError> + 'static,
    AS: AuxStore + Send + Sync + 'static,
    BlockNumber: From<<Block::Header as Header>::Number>,
{
    /// Create new Subspace slot worker
    pub fn new(
        SubspaceSlotWorkerOptions {
            client,
            env,
            block_import,
            sync_oracle,
            justification_sync_link,
            force_authoring,
            backoff_authoring_blocks,
            subspace_link,
            segment_headers_store,
            block_proposal_slot_portion,
            max_block_proposal_slot_portion,
            pot_verifier,
        }: SubspaceSlotWorkerOptions<Block, Client, E, SO, L, BS, AS>,
    ) -> Self {
        Self {
            client: client.clone(),
            block_import,
            env,
            sync_oracle,
            justification_sync_link,
            force_authoring,
            backoff_authoring_blocks,
            subspace_link,
            block_proposal_slot_portion,
            max_block_proposal_slot_portion,
            segment_headers_store,
            pending_solutions: Default::default(),
            pot_checkpoints: Default::default(),
            pot_verifier,
            _pos_table: PhantomData::<PosTable>,
        }
    }

    async fn sign_reward(
        &self,
        hash: Blake3Hash,
        public_key_hash: Blake3Hash,
    ) -> Result<RewardSignature, ConsensusError> {
        let (signature_sender, mut signature_receiver) =
            tracing_unbounded("subspace_signature_signing_stream", 100);

        self.subspace_link
            .reward_signing_notification_sender
            .notify(|| RewardSigningNotification {
                hash,
                public_key_hash,
                signature_sender,
            });

        while let Some(signature) = signature_receiver.next().await {
            if !is_reward_signature_valid(&hash, &signature, &public_key_hash) {
                warn!(
                    %hash,
                    "Received invalid signature for reward"
                );
                continue;
            }

            return Ok(signature);
        }

        Err(ConsensusError::CannotSign(format!(
            "Farmer didn't sign reward. Public key hash: {public_key_hash:?}"
        )))
    }
}

/// Extract solution range for block, given ID of the parent block.
pub(crate) fn extract_solution_range_for_block<Block, Client>(
    client: &Client,
    parent_hash: Block::Hash,
) -> Result<SolutionRange, ApiError>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block>,
    Client::Api: SubspaceApi<Block>,
{
    client
        .runtime_api()
        .solution_ranges(parent_hash)
        .map(|solution_ranges| solution_ranges.next.unwrap_or(solution_ranges.current))
}
