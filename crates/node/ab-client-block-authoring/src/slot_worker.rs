//! Slot worker drives block and vote production based on slots produced in
//! [`ab_client_proof_of_time`].

use ab_client_api::{ChainInfo, ChainSyncStatus};
use ab_client_block_builder::{BlockBuilder, ConsensusConstants};
use ab_client_block_import::BlockImport;
use ab_client_block_import::segment_headers_store::SegmentHeadersStore;
use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::source::{PotSlotInfo, PotSlotInfoStream};
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, GenericBlockHeader, OwnedBlockHeaderSeal,
    SharedBlockHeader,
};
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBeaconChainBlock};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{
    PotCheckpoints, PotOutput, PotParametersChange, PotSeed, SlotNumber,
};
use ab_core_primitives::sectors::SectorId;
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::solutions::{
    Solution, SolutionRange, SolutionVerifyError, SolutionVerifyParams,
    SolutionVerifyPieceCheckParams,
};
use ab_proof_of_space::Table;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use tokio::sync::broadcast;
use tracing::{debug, error, info, trace, warn};

/// Large enough size for any practical purposes, there shouldn't be even this many solutions.
const PENDING_SOLUTIONS_CHANNEL_CAPACITY: usize = 10;

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
/// Notification with a pre-seal hash that needs to be sealed (signed) to create a block and receive
/// a block reward
#[derive(Debug)]
pub struct BlockSealNotification {
    /// Hash to be signed.
    pub pre_seal_hash: Blake3Hash,
    /// Public key hash of the plot identity that should create signature
    pub public_key_hash: Blake3Hash,
    /// Sender that can be used to send the seal
    pub seal_sender: oneshot::Sender<OwnedBlockHeaderSeal>,
}

#[derive(Debug)]
pub struct ClaimedSlot {
    /// Consensus info for block header
    pub consensus_info: BlockHeaderConsensusInfo,
    /// Proof of time seed, the input for computing checkpoints
    pub seed: PotSeed,
    /// Proof of time checkpoints from after future proof of parent block to current block's
    /// future proof (inclusive)
    pub checkpoints: Vec<PotCheckpoints>,
}

/// Parameters for [`SubspaceSlotWorker`]
#[derive(Debug)]
pub struct SubspaceSlotWorkerOptions<BB, BI, BCI, CI, CSS> {
    /// Builder that can create a new block
    pub block_builder: BB,
    /// Block import to import the block created by block builder
    pub block_import: BI,
    /// Beacon chain info
    pub beacon_chain_info: BCI,
    /// Chain info
    pub chain_info: CI,
    /// Chain sync status
    pub chain_sync_status: CSS,
    /// Force authoring of blocks even if we are offline
    pub force_authoring: bool,
    /// Sender for new slot notifications
    pub new_slot_notification_sender: mpsc::Sender<NewSlotNotification>,
    /// Sender for block sealing notifications
    pub block_sealing_notification_sender: mpsc::Sender<BlockSealNotification>,
    // TODO: Should be super segments instead for verification purposes
    /// Persistent storage of segment headers
    pub segment_headers_store: SegmentHeadersStore,
    /// Consensus constants
    pub consensus_constants: ConsensusConstants,
    /// Proof of time verifier
    pub pot_verifier: PotVerifier,
}

/// Subspace slot worker responsible for block and vote production
#[derive(Debug)]
pub struct SubspaceSlotWorker<PosTable, Block, BB, BI, BCI, CI, CSS> {
    block_builder: BB,
    block_import: BI,
    beacon_chain_info: BCI,
    chain_info: CI,
    chain_sync_status: CSS,
    force_authoring: bool,
    new_slot_notification_sender: mpsc::Sender<NewSlotNotification>,
    block_sealing_notification_sender: mpsc::Sender<BlockSealNotification>,
    segment_headers_store: SegmentHeadersStore,
    /// Solution receivers for challenges that were sent to farmers and expected to be received
    /// eventually
    pending_solutions: BTreeMap<SlotNumber, mpsc::Receiver<Solution>>,
    /// Collection of PoT slots that can be retrieved later if needed by block production
    pot_checkpoints: BTreeMap<SlotNumber, PotCheckpoints>,
    consensus_constants: ConsensusConstants,
    pot_verifier: PotVerifier,
    _pos_table: PhantomData<(PosTable, Block)>,
}

impl<PosTable, Block, BB, BI, BCI, CI, CSS>
    SubspaceSlotWorker<PosTable, Block, BB, BI, BCI, CI, CSS>
where
    PosTable: Table,
    Block: GenericOwnedBlock,
    BB: BlockBuilder<Block>,
    BI: BlockImport<Block>,
    BCI: ChainInfo<OwnedBeaconChainBlock>,
    CI: ChainInfo<Block>,
    CSS: ChainSyncStatus,
{
    async fn claim_slot(
        &mut self,
        _parent_block_root: &BlockRoot,
        parent_header: &SharedBlockHeader<'_>,
        parent_beacon_chain_header: &BeaconChainHeader<'_>,
        slot: SlotNumber,
    ) -> Option<ClaimedSlot> {
        let parent_number = parent_header.prefix.number;
        let parent_slot = parent_header.consensus_info.slot;

        if slot <= parent_slot {
            debug!(
                "Skipping claiming slot {slot} it must be higher than parent slot {parent_slot}",
            );

            return None;
        } else {
            debug!(%slot, "Attempting to claim slot");
        }

        let solution_range = parent_beacon_chain_header
            .consensus_parameters
            .next_solution_range
            .unwrap_or(
                parent_beacon_chain_header
                    .consensus_parameters
                    .fixed_parameters
                    .solution_range,
            );

        let consensus_parameters = &parent_beacon_chain_header.consensus_parameters;
        let pot_parameters_change = consensus_parameters
            .pot_parameters_change
            .copied()
            .map(PotParametersChange::from);
        let parent_future_slot = if parent_number == BlockNumber::ZERO {
            parent_slot
        } else {
            parent_slot + self.consensus_constants.block_authoring_delay
        };

        let (proof_of_time, future_proof_of_time, checkpoints) = {
            // Remove checkpoints from old slots we will not need anymore
            self.pot_checkpoints
                .retain(|&stored_slot, _checkpoints| stored_slot > parent_slot);

            let proof_of_time = self.pot_checkpoints.get(&slot)?.output();

            // Future slot for which proof must be available before authoring block at this slot
            let future_slot = slot + self.consensus_constants.block_authoring_delay;

            let pot_input = if parent_number == BlockNumber::ZERO {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: consensus_parameters.fixed_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    consensus_parameters.fixed_parameters.slot_iterations,
                    parent_slot,
                    parent_header.consensus_info.proof_of_time,
                    &pot_parameters_change,
                )
            };

            // Ensure proof of time is valid according to parent block
            if !self.pot_verifier.is_output_valid(
                pot_input,
                slot - parent_slot,
                proof_of_time,
                pot_parameters_change,
            ) {
                warn!(
                    %slot,
                    ?pot_input,
                    consensus_info = ?parent_header.consensus_info,
                    "Proof of time is invalid, skipping block authoring at slot"
                );
                return None;
            }

            let mut checkpoints_pot_input = if parent_number == BlockNumber::ZERO {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: consensus_parameters.fixed_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    consensus_parameters.fixed_parameters.slot_iterations,
                    parent_future_slot,
                    parent_header.consensus_info.future_proof_of_time,
                    &pot_parameters_change,
                )
            };

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
                    &pot_parameters_change,
                );
            }

            let future_proof_of_time = checkpoints
                .last()
                .expect("Never empty, there is at least one slot between blocks; qed")
                .output();

            (proof_of_time, future_proof_of_time, checkpoints)
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

        let mut maybe_consensus_info = None;

        // TODO: Consider skipping most/all checks here and do them in block import instead
        while let Some(solution) = solution_receiver.next().await {
            let sector_id = SectorId::new(
                &solution.public_key_hash,
                solution.sector_index,
                solution.history_size,
            );

            // TODO: Query it from an actual chain
            // let history_size = runtime_api.history_size(parent_block_root).ok()?;
            // let max_pieces_in_sector = runtime_api.max_pieces_in_sector(parent_block_root).ok()?;
            let history_size = HistorySize::ONE;
            let max_pieces_in_sector = 1000;

            let segment_index = sector_id
                .derive_piece_index(
                    solution.piece_offset,
                    solution.history_size,
                    max_pieces_in_sector,
                    self.consensus_constants.recent_segments,
                    self.consensus_constants.recent_history_fraction,
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
                .sector_expiration_check(self.consensus_constants.min_sector_lifetime)
            {
                Some(sector_expiration_check) => sector_expiration_check.segment_index(),
                None => {
                    continue;
                }
            };
            let sector_expiration_check_segment_root = self
                .segment_headers_store
                .get_segment_header(sector_expiration_check_segment_index)
                .map(|segment_header| segment_header.segment_root);

            let solution_verification_result = solution.verify::<PosTable>(
                slot,
                &SolutionVerifyParams {
                    proof_of_time,
                    solution_range,
                    piece_check_params: Some(SolutionVerifyPieceCheckParams {
                        max_pieces_in_sector,
                        segment_root,
                        recent_segments: self.consensus_constants.recent_segments,
                        recent_history_fraction: self.consensus_constants.recent_history_fraction,
                        min_sector_lifetime: self.consensus_constants.min_sector_lifetime,
                        current_history_size: history_size,
                        sector_expiration_check_segment_root,
                    }),
                },
            );

            match solution_verification_result {
                Ok(()) => {
                    if maybe_consensus_info.is_none() {
                        info!(%slot, "🚜 Claimed slot");
                        maybe_consensus_info.replace(BlockHeaderConsensusInfo {
                            slot,
                            proof_of_time,
                            future_proof_of_time,
                            solution,
                        });
                    } else {
                        info!(
                            %slot,
                            "Skipping solution that has quality sufficient for block because \
                            slot has already been claimed",
                        );
                    }
                }
                Err(error @ SolutionVerifyError::OutsideSolutionRange { .. }) => {
                    // Solution range might have just adjusted, but when farmer was auditing they
                    // didn't know about this, so downgrade warning to debug message
                    if parent_beacon_chain_header
                        .consensus_parameters
                        .next_solution_range
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

        maybe_consensus_info.map(|consensus_info| ClaimedSlot {
            consensus_info,
            checkpoints,
        })
    }

    /// Create new Subspace slot worker
    pub fn new(
        SubspaceSlotWorkerOptions {
            block_builder,
            block_import,
            beacon_chain_info,
            chain_info,
            chain_sync_status,
            force_authoring,
            new_slot_notification_sender,
            block_sealing_notification_sender,
            segment_headers_store,
            consensus_constants,
            pot_verifier,
        }: SubspaceSlotWorkerOptions<BB, BI, BCI, CI, CSS>,
    ) -> Self {
        Self {
            block_builder,
            block_import,
            beacon_chain_info,
            chain_info,
            chain_sync_status,
            force_authoring,
            new_slot_notification_sender,
            block_sealing_notification_sender,
            segment_headers_store,
            pending_solutions: BTreeMap::new(),
            pot_checkpoints: BTreeMap::new(),
            consensus_constants,
            pot_verifier,
            _pos_table: PhantomData,
        }
    }

    /// Run slot worker
    pub async fn run(mut self, mut slot_info_stream: PotSlotInfoStream) {
        let mut maybe_last_processed_slot = None;

        loop {
            let PotSlotInfo { slot, checkpoints } = match slot_info_stream.recv().await {
                Ok(slot_info) => slot_info,
                Err(error) => match error {
                    broadcast::error::RecvError::Closed => {
                        info!("No Slot info senders available. Exiting slot worker.");
                        return;
                    }
                    broadcast::error::RecvError::Lagged(skipped_notifications) => {
                        debug!(
                            "Slot worker is lagging. Skipped {} slot notification(s)",
                            skipped_notifications
                        );
                        continue;
                    }
                },
            };
            if let Some(last_processed_slot) = maybe_last_processed_slot
                && last_processed_slot >= slot
            {
                // Already processed
                continue;
            }
            maybe_last_processed_slot.replace(slot);

            self.store_checkpoints(slot, checkpoints);

            let best_beacon_chain_header = self.beacon_chain_info.best_header();
            let best_beacon_chain_header = best_beacon_chain_header.header();
            let best_header = self.chain_info.best_header();
            let best_header = best_header.header();
            let best_root = best_header.root();

            self.on_new_slot(slot, checkpoints, &best_root, &best_beacon_chain_header);

            if self.chain_sync_status.is_syncing() {
                debug!(%slot, "Skipping proposal slot due to sync");
                continue;
            }

            // Slots that we claim must be `block_authoring_delay` behind the best slot we know of
            let Some(slot_to_claim) =
                slot.checked_sub(self.consensus_constants.block_authoring_delay)
            else {
                trace!("Skipping very early slot during chain start");
                continue;
            };

            self.produce_block(
                slot_to_claim,
                &best_root,
                &best_header,
                &best_beacon_chain_header,
            )
            .await;
        }
    }

    /// Handle new slot: store checkpoints and generate notification for farmer
    fn store_checkpoints(&mut self, slot: SlotNumber, checkpoints: PotCheckpoints) {
        // Remove checkpoints from future slots, if present they are out of date anyway
        self.pot_checkpoints
            .retain(|&stored_slot, _checkpoints| stored_slot < slot);

        self.pot_checkpoints.insert(slot, checkpoints);
    }

    /// Handle new slot: store checkpoints and generate notification for farmer
    fn on_new_slot(
        &mut self,
        slot: SlotNumber,
        checkpoints: PotCheckpoints,
        _best_root: &BlockRoot,
        best_beacon_chain_header: &BeaconChainHeader<'_>,
    ) {
        if self.chain_sync_status.is_syncing() {
            debug!("Skipping farming slot {slot} due to sync");
            return;
        }

        let proof_of_time = checkpoints.output();

        // NOTE: Best hash is not necessarily going to be the parent of corresponding block, but
        // solution range shouldn't be too far off
        let solution_range = best_beacon_chain_header
            .consensus_parameters
            .next_solution_range
            .unwrap_or(
                best_beacon_chain_header
                    .consensus_parameters
                    .fixed_parameters
                    .solution_range,
            );
        let new_slot_info = NewSlotInfo {
            slot,
            proof_of_time,
            solution_range,
        };
        let (solution_sender, solution_receiver) =
            mpsc::channel(PENDING_SOLUTIONS_CHANNEL_CAPACITY);

        if let Err(error) = self
            .new_slot_notification_sender
            .try_send(NewSlotNotification {
                new_slot_info,
                solution_sender,
            })
        {
            warn!(%error, "Failed to send new slot notification");
        }

        self.pending_solutions.insert(slot, solution_receiver);
    }

    /// Called with slot for which block needs to be produced (if suitable solution was found)
    async fn produce_block(
        &mut self,
        slot: SlotNumber,
        parent_block_root: &BlockRoot,
        parent_header: &<Block::Header as GenericOwnedBlockHeader>::Header<'_>,
        parent_beacon_chain_header: &BeaconChainHeader<'_>,
    ) -> Option<()> {
        if !self.force_authoring && self.chain_sync_status.is_offline() {
            debug!("Skipping slot, waiting for the network");

            return None;
        }

        let claimed_slot = self
            .claim_slot(
                parent_block_root,
                parent_header,
                parent_beacon_chain_header,
                slot,
            )
            .await?;

        debug!(%slot, "Starting block authorship");

        let seal_block = {
            let block_sealing_notification_sender = &mut self.block_sealing_notification_sender;
            let public_key_hash = claimed_slot.consensus_info.solution.public_key_hash;

            move |pre_seal_hash| async move {
                let (seal_sender, seal_receiver) = oneshot::channel::<OwnedBlockHeaderSeal>();

                if let Err(error) =
                    block_sealing_notification_sender.try_send(BlockSealNotification {
                        pre_seal_hash,
                        public_key_hash,
                        seal_sender,
                    })
                {
                    warn!(%error, "Failed to send block sealing notification");
                }

                seal_receiver.await.ok()
            }
        };
        let block = match self
            .block_builder
            .build(
                parent_block_root,
                parent_header,
                &claimed_slot.consensus_info,
                &claimed_slot.checkpoints,
                seal_block,
            )
            .await
        {
            Ok(block) => block,
            Err(error) => {
                error!(%slot, %parent_block_root, %error, "Failed to build block");
                return None;
            }
        };

        let header = block.header();
        let header = header.header();
        info!(
            number = %header.prefix.number,
            root = %header.root(),
            "🔖 Built new block",
        );

        match self.block_import.import(block) {
            Ok(()) => {
                // Nothing else to do
            }
            Err(error) => {
                warn!(%parent_block_root, %error, "Failed to import new block");
            }
        }

        Some(())
    }
}
