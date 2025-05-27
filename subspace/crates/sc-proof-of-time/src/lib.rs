//! Subspace proof of time implementation.

pub mod source;
pub mod verifier;

use crate::source::{PotSlotInfo, PotSlotInfoStream};
use ab_core_primitives::pot::{
    PotCheckpoints, PotOutput, PotParametersChange, PotSeed, SlotDuration, SlotNumber,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sc_consensus_slots::{SimpleSlotWorker, SimpleSlotWorkerToSlotWorker, SlotInfo, SlotWorker};
use scale_info::TypeInfo;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::{SelectChain, SyncOracle};
use sp_consensus_slots::Slot;
use sp_consensus_subspace::SubspaceApi;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::{Block as BlockT, Header};
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info, trace};

/// Next slot input for proof of time evaluation
#[derive(Debug, Copy, Clone, PartialEq, Eq, Decode, Encode, TypeInfo, MaxEncodedLen)]
pub struct PotNextSlotInput {
    /// Slot number
    pub slot: SlotNumber,
    /// Slot iterations for this slot
    pub slot_iterations: NonZeroU32,
    /// Seed for this slot
    pub seed: PotSeed,
}

impl PotNextSlotInput {
    /// Derive next slot input while taking parameters change into account.
    ///
    /// NOTE: `base_slot_iterations` doesn't have to be parent block, just something that is after
    /// prior parameters change (if any) took effect, in most cases this value corresponds to parent
    /// block's slot.
    pub fn derive(
        base_slot_iterations: NonZeroU32,
        parent_slot: SlotNumber,
        parent_output: PotOutput,
        pot_parameters_change: &Option<PotParametersChange>,
    ) -> Self {
        let next_slot = parent_slot + SlotNumber::ONE;
        let slot_iterations;
        let seed;

        // The change to number of iterations might have happened before `next_slot`
        if let Some(parameters_change) = pot_parameters_change
            && parameters_change.slot <= next_slot
        {
            slot_iterations = parameters_change.slot_iterations;
            // Only if entropy injection happens exactly on next slot we need to mix it in
            if parameters_change.slot == next_slot {
                seed = parent_output.seed_with_entropy(&parameters_change.entropy);
            } else {
                seed = parent_output.seed();
            }
        } else {
            slot_iterations = base_slot_iterations;
            seed = parent_output.seed();
        }

        Self {
            slot: next_slot,
            slot_iterations,
            seed,
        }
    }
}

pub trait PotSlotWorker<Block>
where
    Block: BlockT,
{
    /// Called when new proof of time is available for slot.
    ///
    /// NOTE: Can be called more than once in case of reorgs to override old slots.
    fn on_proof(&mut self, slot: SlotNumber, checkpoints: PotCheckpoints);
}

/// Start a new slot worker.
///
/// Every time a new slot is triggered, `worker.on_slot` is called and the future it returns is
/// polled until completion, unless we are major syncing.
pub async fn start_slot_worker<Block, Client, SC, Worker, SO, CIDP>(
    slot_duration: SlotDuration,
    client: Arc<Client>,
    select_chain: SC,
    worker: Worker,
    sync_oracle: SO,
    create_inherent_data_providers: CIDP,
    mut slot_info_stream: PotSlotInfoStream,
) where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Client::Api: SubspaceApi<Block>,
    SC: SelectChain<Block>,
    Worker: PotSlotWorker<Block> + SimpleSlotWorker<Block> + Send + Sync,
    SO: SyncOracle + Send,
    CIDP: CreateInherentDataProviders<Block, ()> + Send + 'static,
{
    let best_hash = client.info().best_hash;
    let runtime_api = client.runtime_api();
    let block_authoring_delay = match runtime_api.chain_constants(best_hash) {
        Ok(chain_constants) => chain_constants.block_authoring_delay(),
        Err(error) => {
            error!(%error, "Failed to retrieve chain constants from runtime API");
            return;
        }
    };

    let slot_duration = slot_duration.as_duration();

    let mut worker = SimpleSlotWorkerToSlotWorker(worker);

    let mut maybe_last_proven_slot = None;

    loop {
        let PotSlotInfo { slot, checkpoints } = match slot_info_stream.recv().await {
            Ok(slot_info) => slot_info,
            Err(err) => match err {
                RecvError::Closed => {
                    info!("No Slot info senders available. Exiting slot worker.");
                    return;
                }
                RecvError::Lagged(skipped_notifications) => {
                    debug!(
                        "Slot worker is lagging. Skipped {} slot notification(s)",
                        skipped_notifications
                    );
                    continue;
                }
            },
        };
        if let Some(last_proven_slot) = maybe_last_proven_slot
            && last_proven_slot >= slot
        {
            // Already processed
            continue;
        }
        maybe_last_proven_slot.replace(slot);

        worker.0.on_proof(slot, checkpoints);

        if sync_oracle.is_major_syncing() {
            debug!(%slot, "Skipping proposal slot due to sync");
            continue;
        }

        // Slots that we claim must be `block_authoring_delay` behind the best slot we know of
        let Some(slot_to_claim) = slot.checked_sub(block_authoring_delay) else {
            trace!("Skipping very early slot during chain start");
            continue;
        };

        let best_header = match select_chain.best_chain().await {
            Ok(best_header) => best_header,
            Err(error) => {
                error!(
                    %error,
                    "Unable to author block in slot. No best block header.",
                );

                continue;
            }
        };

        let inherent_data_providers = match create_inherent_data_providers
            .create_inherent_data_providers(best_header.hash(), ())
            .await
        {
            Ok(inherent_data_providers) => inherent_data_providers,
            Err(error) => {
                error!(
                    %error,
                    "Unable to author block in slot. Failure creating inherent data provider.",
                );

                continue;
            }
        };

        let _ = worker
            .on_slot(SlotInfo::new(
                Slot::from(slot_to_claim.as_u64()),
                Box::new(inherent_data_providers),
                slot_duration,
                best_header,
                None,
            ))
            .await;
    }
}
