use crate::{ConsensusConstants, PotConsensusConstants};
use ab_client_api::ChainInfo;
use ab_core_primitives::block::header::{
    BlockHeaderConsensusParameters, BlockHeaderFixedConsensusParameters,
    BlockHeaderPotParametersChange,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::pot::{PotParametersChange, SlotNumber};
use ab_core_primitives::solutions::SolutionRange;
use std::num::NonZeroU32;

struct SolutionRanges {
    current: SolutionRange,
    next: Option<SolutionRange>,
}

struct PotInfo {
    slot_iterations: NonZeroU32,
    parameters_change: Option<PotParametersChange>,
}

/// Derived consensus parameters, to be eventually turned into
/// [`OwnedBlockHeaderConsensusParameters`]
///
/// [`OwnedBlockHeaderConsensusParameters`]: ab_core_primitives::block::header::OwnedBlockHeaderConsensusParameters
#[derive(Debug, Copy, Clone)]
pub struct DerivedConsensusParameters {
    /// Consensus parameters that are always present
    pub fixed_parameters: BlockHeaderFixedConsensusParameters,
    /// Solution range for the next block/era (if any)
    pub next_solution_range: Option<SolutionRange>,
    /// Change of parameters to apply to the proof of time chain (if any)
    pub pot_parameters_change: Option<BlockHeaderPotParametersChange>,
}

/// Error for [`derive_consensus_parameters`]
#[derive(Debug, thiserror::Error)]
pub enum DeriveConsensusParametersError {
    /// Failed to get ancestor header
    #[error("Failed to get ancestor header")]
    GetAncestorHeader,
}

// TODO: Another domain-specific abstraction over `ChainInfo`, which will be implemented for
//  `ChainInfo`, but could also be implemented in simpler way directly for tests without dealing
//  with complete headers, etc.
pub fn derive_consensus_parameters<CI>(
    consensus_constants: &ConsensusConstants,
    chain_info: &CI,
    parent_block_root: &BlockRoot,
    parent_consensus_parameters: &BlockHeaderConsensusParameters<'_>,
    parent_slot: SlotNumber,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<DerivedConsensusParameters, DeriveConsensusParametersError>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    let solution_ranges = derive_solution_ranges(
        consensus_constants.era_duration,
        consensus_constants.slot_probability,
        chain_info,
        parent_block_root,
        parent_consensus_parameters.fixed_parameters.solution_range,
        parent_consensus_parameters.next_solution_range,
        block_number,
        slot,
    )?;
    let pot_info = derive_pot_info(
        &consensus_constants.pot,
        chain_info,
        parent_block_root,
        parent_slot,
        parent_consensus_parameters.fixed_parameters.slot_iterations,
        parent_consensus_parameters
            .pot_parameters_change
            .copied()
            .map(PotParametersChange::from),
        block_number,
        slot,
    )?;

    Ok(DerivedConsensusParameters {
        fixed_parameters: BlockHeaderFixedConsensusParameters {
            solution_range: solution_ranges.current,
            slot_iterations: pot_info.slot_iterations,
        },
        next_solution_range: solution_ranges.next,
        pot_parameters_change: pot_info
            .parameters_change
            .map(BlockHeaderPotParametersChange::from),
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "Explicit minimal input for better testability"
)]
fn derive_solution_ranges<CI>(
    era_duration: BlockNumber,
    slot_probability: (u64, u64),
    chain_info: &CI,
    parent_block_root: &BlockRoot,
    solution_range: SolutionRange,
    next_solution_range: Option<SolutionRange>,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<SolutionRanges, DeriveConsensusParametersError>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    if let Some(next_solution_range) = next_solution_range {
        return Ok(SolutionRanges {
            current: next_solution_range,
            next: None,
        });
    }

    let next_solution_range = if block_number.as_u64().is_multiple_of(era_duration.as_u64())
        && block_number > era_duration
    {
        let era_start_block = block_number.saturating_sub(era_duration);
        let era_start_slot = chain_info
            .ancestor_header(era_start_block, parent_block_root)
            .ok_or(DeriveConsensusParametersError::GetAncestorHeader)?
            .header()
            .consensus_info
            .slot;

        Some(solution_range.derive_next(
            slot.saturating_sub(era_start_slot),
            slot_probability,
            era_duration,
        ))
    } else {
        None
    };

    Ok(SolutionRanges {
        current: solution_range,
        next: next_solution_range,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "Explicit minimal input for better testability"
)]
fn derive_pot_info<CI>(
    pot_consensus_constants: &PotConsensusConstants,
    chain_info: &CI,
    parent_block_root: &BlockRoot,
    parent_slot: SlotNumber,
    parent_slot_iterations: NonZeroU32,
    parent_parameters_change: Option<PotParametersChange>,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<PotInfo, DeriveConsensusParametersError>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    let pot_entropy_injection_interval = pot_consensus_constants.entropy_injection_interval;
    let pot_entropy_injection_lookback_depth =
        pot_consensus_constants.entropy_injection_lookback_depth;
    let pot_entropy_injection_delay = pot_consensus_constants.entropy_injection_delay;

    // Value right after parent block's slot
    let slot_iterations = if let Some(change) = &parent_parameters_change
        && change.slot <= parent_slot.saturating_add(SlotNumber::ONE)
    {
        change.slot_iterations
    } else {
        parent_slot_iterations
    };

    let parameters_change = if let Some(change) = parent_parameters_change
        && change.slot > slot
    {
        // Retain previous PoT parameters change if it applies after the block's slot
        Some(change)
    } else {
        let lookback_in_blocks = BlockNumber::new(
            pot_entropy_injection_interval.as_u64()
                * u64::from(pot_entropy_injection_lookback_depth),
        );
        let last_entropy_injection_block_number = BlockNumber::new(
            block_number.as_u64() / pot_entropy_injection_interval.as_u64()
                * pot_entropy_injection_interval.as_u64(),
        );
        let maybe_entropy_source_block_number =
            last_entropy_injection_block_number.checked_sub(lookback_in_blocks);

        // Inject entropy every `pot_entropy_injection_interval` blocks
        if last_entropy_injection_block_number == block_number
            && let Some(entropy_source_block_number) = maybe_entropy_source_block_number
            && entropy_source_block_number > BlockNumber::ZERO
        {
            let entropy = {
                let entropy_source_block_header = chain_info
                    .ancestor_header(entropy_source_block_number, parent_block_root)
                    .ok_or(DeriveConsensusParametersError::GetAncestorHeader)?;
                let entropy_source_block_header = entropy_source_block_header.header();

                entropy_source_block_header
                    .consensus_info
                    .proof_of_time
                    .derive_pot_entropy(&entropy_source_block_header.consensus_info.solution.chunk)
            };

            let target_slot = slot
                .checked_add(pot_entropy_injection_delay)
                .unwrap_or(SlotNumber::MAX);

            Some(PotParametersChange {
                slot: target_slot,
                // TODO: A mechanism to increase (not decrease!) number of iterations if slots
                //  are created too frequently on long enough timescale, maybe based on the same
                //  lookback depth as entropy (would be the cleanest and easiest to explain)
                slot_iterations,
                entropy,
            })
        } else {
            None
        }
    };

    Ok(PotInfo {
        slot_iterations,
        parameters_change,
    })
}
