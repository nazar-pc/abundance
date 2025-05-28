pub mod block_import;
pub mod gossip;

use ab_client_proof_of_time::PotNextSlotInput;
use ab_client_proof_of_time::source::state::PotState;
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::pot::SlotNumber;
use sc_client_api::BlockchainEvents;
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus_subspace::SubspaceApi;
use sp_consensus_subspace::digests::extract_pre_digest;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT, Zero};
use std::sync::Arc;

/// Initialize [`PotState`]
pub fn init_pot_state<Block, Client>(
    client: Arc<Client>,
    pot_verifier: PotVerifier,
) -> Result<PotState, ApiError>
where
    Block: BlockT,
    Client: BlockchainEvents<Block> + HeaderBackend<Block> + ProvideRuntimeApi<Block> + 'static,
    Client::Api: SubspaceApi<Block>,
{
    let best_hash = client.info().best_hash;
    let runtime_api = client.runtime_api();
    let chain_constants = runtime_api.chain_constants(best_hash)?;

    let best_header = client
        .header(best_hash)?
        .ok_or_else(|| ApiError::UnknownBlock(format!("Parent block {best_hash} not found")))?;
    let best_pre_digest =
        extract_pre_digest(&best_header).map_err(|error| ApiError::Application(error.into()))?;

    let parent_slot = if best_header.number().is_zero() {
        SlotNumber::ZERO
    } else {
        // The best one seen
        best_pre_digest.slot + chain_constants.block_authoring_delay()
    };

    let pot_parameters = runtime_api.pot_parameters(best_hash)?;
    let maybe_next_parameters_change = pot_parameters.next_change;

    let pot_input = if best_header.number().is_zero() {
        PotNextSlotInput {
            slot: parent_slot + SlotNumber::ONE,
            slot_iterations: pot_parameters.slot_iterations,
            seed: pot_verifier.genesis_seed(),
        }
    } else {
        PotNextSlotInput::derive(
            pot_parameters.slot_iterations,
            parent_slot,
            best_pre_digest.pot_info.future_proof_of_time,
            &maybe_next_parameters_change,
        )
    };

    Ok(PotState::new(
        pot_input,
        maybe_next_parameters_change,
        pot_verifier,
    ))
}
