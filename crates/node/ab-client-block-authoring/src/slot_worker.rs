//! Slot worker drives block and vote production based on slots produced in
//! [`ab_client_proof_of_time`].

use crate::{BlockProducer, ClaimedSlot};
use ab_client_api::{ChainInfo, ChainSyncStatus};
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::shard_membership_entropy_source;
use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::source::{PotSlotInfo, PotSlotInfoStream};
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, OwnedBlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, PotParametersChange, SlotNumber};
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{ShardMembershipEntropy, Solution, SolutionRange};
use ab_proof_of_space::Table;
use futures::StreamExt;
use futures::channel::{mpsc, oneshot};
use send_future::SendFuture;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, error, info, trace, warn};

/// Large enough size for any practical purposes, there shouldn't be even this many solutions.
const PENDING_SOLUTIONS_CHANNEL_CAPACITY: usize = 10;
const BLOCK_SEALING_TIMEOUT: Duration = Duration::from_millis(500);

/// Information about a new slot that just arrived
#[derive(Debug, Copy, Clone)]
pub struct NewSlotInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// The PoT output for `slot`
    pub proof_of_time: PotOutput,
    /// Acceptable solution range for block authoring
    pub solution_range: SolutionRange,
    /// Shard membership entropy
    pub shard_membership_entropy: ShardMembershipEntropy,
    /// The number of shards in the network
    pub num_shards: NumShards,
}

/// New slot notification with slot information and sender for a solution for the slot.
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

/// Options for [`SlotWorker`]
#[derive(Debug)]
pub struct SlotWorkerOptions<BP, BCI, CSS> {
    /// Producer of a new block
    pub block_producer: BP,
    /// Beacon chain info
    pub beacon_chain_info: BCI,
    /// Chain sync status
    pub chain_sync_status: CSS,
    /// Force authoring of blocks even if we are offline
    pub force_authoring: bool,
    /// Sender for new slot notifications
    pub new_slot_notification_sender: mpsc::Sender<NewSlotNotification>,
    /// Sender for block sealing notifications
    pub block_sealing_notification_sender: mpsc::Sender<BlockSealNotification>,
    /// Consensus constants
    pub consensus_constants: ConsensusConstants,
    /// Proof of time verifier
    pub pot_verifier: PotVerifier,
}

/// Slot worker responsible for block production
#[derive(Debug)]
pub struct SlotWorker<PosTable, BP, BCI, CSS> {
    block_producer: BP,
    beacon_chain_info: BCI,
    chain_sync_status: CSS,
    force_authoring: bool,
    new_slot_notification_sender: mpsc::Sender<NewSlotNotification>,
    block_sealing_notification_sender: mpsc::Sender<BlockSealNotification>,
    /// Solution receivers for challenges that were sent to farmers and expected to be received
    /// eventually
    pending_solutions: BTreeMap<SlotNumber, mpsc::Receiver<Solution>>,
    /// Collection of PoT slots that can be retrieved later if needed by block production
    pot_checkpoints: BTreeMap<SlotNumber, PotCheckpoints>,
    consensus_constants: ConsensusConstants,
    pot_verifier: PotVerifier,
    _pos_table: PhantomData<PosTable>,
}

impl<PosTable, BP, BCI, CSS> SlotWorker<PosTable, BP, BCI, CSS>
where
    PosTable: Table,
    BP: BlockProducer,
    BCI: ChainInfo<OwnedBeaconChainBlock>,
    CSS: ChainSyncStatus,
{
    /// Create a new slot worker
    pub fn new(
        SlotWorkerOptions {
            block_producer,
            beacon_chain_info,
            chain_sync_status,
            force_authoring,
            new_slot_notification_sender,
            block_sealing_notification_sender,
            consensus_constants,
            pot_verifier,
        }: SlotWorkerOptions<BP, BCI, CSS>,
    ) -> Self {
        Self {
            block_producer,
            beacon_chain_info,
            chain_sync_status,
            force_authoring,
            new_slot_notification_sender,
            block_sealing_notification_sender,
            pending_solutions: BTreeMap::new(),
            pot_checkpoints: BTreeMap::new(),
            consensus_constants,
            pot_verifier,
            _pos_table: PhantomData,
        }
    }

    /// Run slot worker
    pub async fn run(mut self, mut slot_info_stream: PotSlotInfoStream) {
        let mut last_processed_slot = SlotNumber::ZERO;

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

            if last_processed_slot >= slot {
                // Already processed
                continue;
            }
            last_processed_slot = slot;

            let best_beacon_chain_header = self.beacon_chain_info.best_header();
            let best_beacon_chain_header = best_beacon_chain_header.header();

            // Store checkpoints
            {
                // Remove checkpoints from future slots, if present they are out of date anyway
                self.pot_checkpoints
                    .retain(|&stored_slot, _checkpoints| stored_slot < slot);

                self.pot_checkpoints.insert(slot, checkpoints);
            }

            if self.chain_sync_status.is_syncing() {
                debug!(%slot, "Skipping farming due to syncing");
                return;
            }

            // TODO: Maybe handle the boundary in some way, like checking already received
            //  solutions (which are waiting for future PoT to produce a block) or send both entropy
            //  sources at the interval boundary
            // NOTE: Beacon chain block number may change before the next block is produced,
            // rendering the entropy source invalid, but it should not happen often.
            let shard_membership_entropy = match shard_membership_entropy_source(
                best_beacon_chain_header.prefix.number + BlockNumber::ONE,
                best_beacon_chain_header,
                self.consensus_constants.shard_rotation_interval,
                self.consensus_constants.shard_rotation_delay,
                &self.beacon_chain_info,
            ) {
                Ok(shard_membership_entropy) => shard_membership_entropy,
                Err(error) => {
                    error!(%error, "Failed to find shard membership entropy");
                    break;
                }
            };

            let proof_of_time = checkpoints.output();

            // Send slot notification to farmers
            {
                let consensus_parameters = best_beacon_chain_header.consensus_parameters();
                // NOTE: Best bock is not necessarily going to be the parent of the corresponding
                // block once it is created, but solution range and number of shards should be the
                // same most of the time
                let solution_range = consensus_parameters
                    .next_solution_range
                    .unwrap_or(consensus_parameters.fixed_parameters.solution_range);
                let new_slot_info = NewSlotInfo {
                    slot,
                    proof_of_time,
                    solution_range,
                    shard_membership_entropy,
                    num_shards: consensus_parameters.fixed_parameters.num_shards,
                };
                let (solution_sender, solution_receiver) =
                    mpsc::channel(PENDING_SOLUTIONS_CHANNEL_CAPACITY);

                if let Err(error) =
                    self.new_slot_notification_sender
                        .try_send(NewSlotNotification {
                            new_slot_info,
                            solution_sender,
                        })
                {
                    warn!(%error, "Failed to send a new slot notification");
                }

                self.pending_solutions.insert(slot, solution_receiver);
            }

            // Slots that we claim must be `block_authoring_delay` behind the best slot we know of
            let Some(slot_to_claim) =
                slot.checked_sub(self.consensus_constants.block_authoring_delay)
            else {
                trace!("Skipping a very early slot during chain start");
                continue;
            };

            if !self.force_authoring && self.chain_sync_status.is_offline() {
                debug!("Skipping slot, waiting for the network");

                continue;
            }

            let Some(claimed_slot) = self
                .claim_slot(best_beacon_chain_header, slot_to_claim)
                .await
            else {
                continue;
            };

            debug!(
                slot = %claimed_slot.consensus_info.slot,
                "Starting block authorship"
            );

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

                    match tokio::time::timeout(BLOCK_SEALING_TIMEOUT, seal_receiver).await {
                        Ok(Ok(seal)) => Some(seal),
                        _ => None,
                    }
                }
            };

            // TODO: `.send()` is a hack for compiler bug, see:
            //  https://github.com/rust-lang/rust/issues/100013#issuecomment-2210995259
            self.block_producer
                .produce_block(claimed_slot, best_beacon_chain_header, seal_block)
                .send()
                .await;
        }
    }

    async fn claim_slot(
        &mut self,
        parent_beacon_chain_header: &BeaconChainHeader<'_>,
        slot: SlotNumber,
    ) -> Option<ClaimedSlot> {
        let parent_number = parent_beacon_chain_header.prefix.number;
        let parent_slot = parent_beacon_chain_header.consensus_info.slot;

        if slot <= parent_slot {
            debug!(
                "Skipping claiming slot {slot}, it must be higher than parent slot {parent_slot}",
            );

            return None;
        } else {
            debug!(%slot, "Attempting to claim a slot");
        }

        let parent_consensus_parameters = parent_beacon_chain_header.consensus_parameters();

        let parent_pot_parameters_change = parent_consensus_parameters
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

            // Future slot for which proof must be available before authoring a block at this slot
            let future_slot = slot + self.consensus_constants.block_authoring_delay;

            let pot_input = if parent_number == BlockNumber::ZERO {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: parent_consensus_parameters.fixed_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    parent_consensus_parameters.fixed_parameters.slot_iterations,
                    parent_slot,
                    parent_beacon_chain_header.consensus_info.proof_of_time,
                    &parent_pot_parameters_change,
                )
            };

            // Ensure proof of time is valid, according to parent block
            if !self.pot_verifier.is_output_valid(
                pot_input,
                slot - parent_slot,
                proof_of_time,
                parent_pot_parameters_change,
            ) {
                warn!(
                    %slot,
                    ?pot_input,
                    consensus_info = ?parent_beacon_chain_header.consensus_info,
                    "Proof of time is invalid, skipping block authoring at the slot"
                );
                return None;
            }

            let mut checkpoints_pot_input = if parent_number == BlockNumber::ZERO {
                PotNextSlotInput {
                    slot: parent_slot + SlotNumber::ONE,
                    slot_iterations: parent_consensus_parameters.fixed_parameters.slot_iterations,
                    seed: self.pot_verifier.genesis_seed(),
                }
            } else {
                PotNextSlotInput::derive(
                    parent_consensus_parameters.fixed_parameters.slot_iterations,
                    parent_future_slot,
                    parent_beacon_chain_header
                        .consensus_info
                        .future_proof_of_time,
                    &parent_pot_parameters_change,
                )
            };

            let mut checkpoints =
                Vec::with_capacity(u64::from(future_slot - parent_future_slot) as usize);

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
                    &parent_pot_parameters_change,
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

        while let Some(solution) = solution_receiver.next().await {
            if maybe_consensus_info.is_none() {
                debug!(%slot, "🚜 Claimed slot");
                maybe_consensus_info.replace(BlockHeaderConsensusInfo {
                    slot,
                    proof_of_time,
                    future_proof_of_time,
                    solution,
                });
            } else {
                debug!(
                    %slot,
                    "Skipping a solution that has quality sufficient for block because \
                    slot has already been claimed",
                );
            }
        }

        maybe_consensus_info.map(|consensus_info| ClaimedSlot {
            consensus_info,
            checkpoints,
        })
    }
}
