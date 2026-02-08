use crate::{ConsensusConstants, PotConsensusConstants};
use ab_client_api::ChainInfo;
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderFixedConsensusParameters, BlockHeaderPotParametersChange,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::pieces::RecordChunk;
use ab_core_primitives::pot::{PotOutput, PotParametersChange, SlotNumber};
use ab_core_primitives::solutions::{ShardMembershipEntropy, SolutionRange};
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
    /// Solution range for the next block/interval (if any)
    pub next_solution_range: Option<SolutionRange>,
    /// Change of parameters to apply to the proof of time chain (if any)
    pub pot_parameters_change: Option<BlockHeaderPotParametersChange>,
}

/// Error for [`derive_consensus_parameters()`]
#[derive(Debug, thiserror::Error)]
pub enum DeriveConsensusParametersError {
    /// Failed to get ancestor header
    #[error("Failed to get ancestor header")]
    GetAncestorHeader,
}

/// A limited subset of [`BlockHeaderConsensusInfo`] for [`derive_consensus_parameters()`]
#[derive(Debug, Clone, Copy)]
pub struct DeriveConsensusParametersConsensusInfo {
    /// Slot number
    pub slot: SlotNumber,
    /// Proof of time for this slot
    pub proof_of_time: PotOutput,
    /// Record chunk used in a solution
    pub solution_record_chunk: RecordChunk,
}

impl DeriveConsensusParametersConsensusInfo {
    pub fn from_consensus_info(consensus_info: &BlockHeaderConsensusInfo) -> Self {
        Self {
            slot: consensus_info.slot,
            proof_of_time: consensus_info.proof_of_time,
            solution_record_chunk: consensus_info.solution.chunk,
        }
    }
}

/// Chain info for [`derive_consensus_parameters()`].
///
/// Must have access to enough parent blocks.
pub trait DeriveConsensusParametersChainInfo: Send + Sync {
    /// Get header of ancestor block number for descendant block root
    fn ancestor_header_consensus_info(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<DeriveConsensusParametersConsensusInfo>;
}

impl<T> DeriveConsensusParametersChainInfo for T
where
    T: ChainInfo<OwnedBeaconChainBlock>,
{
    fn ancestor_header_consensus_info(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<DeriveConsensusParametersConsensusInfo> {
        let header = self.ancestor_header(ancestor_block_number, descendant_block_root)?;

        Some(DeriveConsensusParametersConsensusInfo::from_consensus_info(
            header.header().consensus_info,
        ))
    }
}

pub fn derive_consensus_parameters<BCI>(
    consensus_constants: &ConsensusConstants,
    beacon_chain_info: &BCI,
    parent_block_root: &BlockRoot,
    parent_consensus_parameters: &BlockHeaderConsensusParameters<'_>,
    parent_slot: SlotNumber,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<DerivedConsensusParameters, DeriveConsensusParametersError>
where
    BCI: DeriveConsensusParametersChainInfo,
{
    let solution_ranges = derive_solution_ranges(
        consensus_constants.retarget_interval,
        consensus_constants.slot_probability,
        beacon_chain_info,
        parent_block_root,
        parent_consensus_parameters.fixed_parameters.solution_range,
        parent_consensus_parameters.next_solution_range,
        block_number,
        slot,
    )?;
    let pot_info = derive_pot_info(
        &consensus_constants.pot,
        beacon_chain_info,
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
            num_shards: parent_consensus_parameters.fixed_parameters.num_shards,
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
fn derive_solution_ranges<BCI>(
    retarget_interval: BlockNumber,
    slot_probability: (u64, u64),
    beacon_chain_info: &BCI,
    parent_block_root: &BlockRoot,
    solution_range: SolutionRange,
    next_solution_range: Option<SolutionRange>,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<SolutionRanges, DeriveConsensusParametersError>
where
    BCI: DeriveConsensusParametersChainInfo,
{
    if let Some(next_solution_range) = next_solution_range {
        return Ok(SolutionRanges {
            current: next_solution_range,
            next: None,
        });
    }

    let next_solution_range = if u64::from(block_number)
        .is_multiple_of(u64::from(retarget_interval))
        && block_number > retarget_interval
    {
        let interval_start_block = block_number.saturating_sub(retarget_interval);
        let interval_start_slot = beacon_chain_info
            .ancestor_header_consensus_info(interval_start_block, parent_block_root)
            .ok_or(DeriveConsensusParametersError::GetAncestorHeader)?
            .slot;

        Some(solution_range.derive_next(
            slot.saturating_sub(interval_start_slot),
            slot_probability,
            retarget_interval,
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
fn derive_pot_info<BCI>(
    pot_consensus_constants: &PotConsensusConstants,
    beacon_chain_info: &BCI,
    parent_block_root: &BlockRoot,
    parent_slot: SlotNumber,
    parent_slot_iterations: NonZeroU32,
    parent_parameters_change: Option<PotParametersChange>,
    block_number: BlockNumber,
    slot: SlotNumber,
) -> Result<PotInfo, DeriveConsensusParametersError>
where
    BCI: DeriveConsensusParametersChainInfo,
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
        let lookback_in_blocks = BlockNumber::from(
            u64::from(pot_entropy_injection_interval)
                * u64::from(pot_entropy_injection_lookback_depth),
        );
        let last_entropy_injection_block_number = BlockNumber::from(
            u64::from(block_number) / u64::from(pot_entropy_injection_interval)
                * u64::from(pot_entropy_injection_interval),
        );
        let maybe_entropy_source_block_number =
            last_entropy_injection_block_number.checked_sub(lookback_in_blocks);

        // Inject entropy every `pot_entropy_injection_interval` blocks
        if last_entropy_injection_block_number == block_number
            && let Some(entropy_source_block_number) = maybe_entropy_source_block_number
            && entropy_source_block_number > BlockNumber::ZERO
        {
            let entropy = {
                let consensus_info = beacon_chain_info
                    .ancestor_header_consensus_info(entropy_source_block_number, parent_block_root)
                    .ok_or(DeriveConsensusParametersError::GetAncestorHeader)?;

                consensus_info
                    .proof_of_time
                    .derive_pot_entropy(&consensus_info.solution_record_chunk)
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

/// Chain info for [`shard_membership_entropy_source()`].
///
/// Must have access to enough parent blocks.
pub trait ShardMembershipEntropySourceChainInfo: Send + Sync {
    fn ancestor_header_proof_of_time(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<PotOutput>;
}

impl<T> ShardMembershipEntropySourceChainInfo for T
where
    T: ChainInfo<OwnedBeaconChainBlock>,
{
    fn ancestor_header_proof_of_time(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<PotOutput> {
        let header =
            ChainInfo::ancestor_header(self, ancestor_block_number, descendant_block_root)?;
        Some(header.header().consensus_info.proof_of_time)
    }
}

/// Error for [`shard_membership_entropy_source`]
#[derive(Debug, thiserror::Error)]
pub enum ShardMembershipEntropySourceError {
    /// Failed to find a beacon chain block with the shard membership entropy source
    #[error(
        "Failed to find a beacon chain block {block_number} with the shard membership entropy \
        source"
    )]
    FailedToFindBeaconChainBlock {
        /// Entropy source block number
        block_number: BlockNumber,
    },
}

/// Find shard membership entropy for a specified block number
pub fn shard_membership_entropy_source<BCI>(
    block_number: BlockNumber,
    best_beacon_chain_header: &BeaconChainHeader<'_>,
    shard_rotation_interval: BlockNumber,
    shard_rotation_delay: BlockNumber,
    beacon_chain_info: &BCI,
) -> Result<ShardMembershipEntropy, ShardMembershipEntropySourceError>
where
    BCI: ShardMembershipEntropySourceChainInfo,
{
    let entropy_source_block_number = BlockNumber::from(
        u64::from(block_number.saturating_sub(shard_rotation_delay))
            / u64::from(shard_rotation_interval)
            * u64::from(shard_rotation_interval),
    );

    let proof_of_time = beacon_chain_info
        .ancestor_header_proof_of_time(
            entropy_source_block_number,
            &best_beacon_chain_header.root(),
        )
        .ok_or(
            ShardMembershipEntropySourceError::FailedToFindBeaconChainBlock {
                block_number: entropy_source_block_number,
            },
        )?;

    Ok(proof_of_time.shard_membership_entropy())
}
