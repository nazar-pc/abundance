#[cfg(test)]
mod tests;

use crate::{ConsensusConstants, PotConsensusConstants};
use ab_client_api::{BeaconChainInfo, ChainInfo, ShardSegmentRoot, ShardSegmentRootsError};
use ab_core_primitives::block::body::OwnSegments;
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderFixedConsensusParameters,
    BlockHeaderPotParametersChange,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::RecordChunk;
use ab_core_primitives::pot::{PotOutput, PotParametersChange, SlotNumber};
use ab_core_primitives::segments::{
    LocalSegmentIndex, SegmentIndex, SegmentPosition, SegmentRoot, ShardSegmentRootWithPosition,
    SuperSegment, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::solutions::{ShardMembershipEntropy, SolutionRange};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc as StdArc;
use triomphe::Arc;

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
    /// Get consensus info of the block at `ancestor_block_number` on the chain ending at
    /// `descendant_block_root`.
    ///
    /// If `ancestor_block_number` is the descendant's own number, the descendant's consensus info
    /// is returned. Returns `None` if `ancestor_block_number` is higher than the descendant's
    /// number or if either block is not available.
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
    /// Get proof of time of the block at `ancestor_block_number` on the chain ending at
    /// `descendant_block_root`.
    ///
    /// If `ancestor_block_number` is the descendant's own number, the descendant's proof of time
    /// is returned. Returns `None` if `ancestor_block_number` is higher than the descendant's
    /// number or if either block is not available.
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

/// Find shard membership entropy for a block with a specified block number and parent.
///
/// The entropy source block is looked up among the ancestors of `parent_block_root`, on the chain
/// that the block extends.
pub fn shard_membership_entropy_source<BCI>(
    block_number: BlockNumber,
    parent_block_root: &BlockRoot,
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
        .ancestor_header_proof_of_time(entropy_source_block_number, parent_block_root)
        .ok_or(
            ShardMembershipEntropySourceError::FailedToFindBeaconChainBlock {
                block_number: entropy_source_block_number,
            },
        )?;

    Ok(proof_of_time.shard_membership_entropy())
}

/// Error for [`derive_super_segment_for_block()`]
#[derive(Debug, thiserror::Error)]
pub enum DeriveSuperSegmentForBlockError {
    /// Genesis beacon chain segment header not found
    #[error("Genesis beacon chain segment header not found")]
    GenesisBeaconChainSegmentHeaderNotFound,
    /// Parent super segment header not found
    #[error("Parent super segment header not found for block {block_number}")]
    ParentSuperSegmentHeaderNotFound {
        /// Block number for which the parent super segment header was not found
        block_number: BlockNumber,
    },
    /// Shard segment roots error
    #[error("Shard segment roots error: {error}")]
    ShardSegmentRootsError {
        /// Low-level error
        #[from]
        error: ShardSegmentRootsError,
    },
    /// Too many segments
    #[error("Too many segments: {extra_segment_roots} extra segment roots")]
    TooManySegments {
        /// Number of extra segment roots
        extra_segment_roots: usize,
    },
}

/// Chain info for [`derive_super_segment_for_block()`].
///
/// Must have access to enough parent blocks.
pub trait DeriveSuperSegmentForBlockChainInfo: Send + Sync {
    /// Get genesis segment root of the beacon chain
    fn get_genesis_segment_root(&self) -> Result<SegmentRoot, DeriveSuperSegmentForBlockError>;

    /// Get segment roots that are expected to be included at specified block number
    fn segment_roots_for_block(
        &self,
        block_number: BlockNumber,
    ) -> impl ExactSizeIterator<Item = ShardSegmentRoot> + Send + Sync + 'static;

    /// Returns the previous super segment header for the block built with the specified number
    fn previous_super_segment_header(
        &self,
        block_number: BlockNumber,
    ) -> Option<SuperSegmentHeader>;

    /// Returns intermediate and leaf shard segment roots included in the block at
    /// `ancestor_block_number` on the chain ending at `descendant_block_root`
    fn shard_segment_roots(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Result<Arc<[ShardSegmentRoot]>, ShardSegmentRootsError>;
}

impl<T> DeriveSuperSegmentForBlockChainInfo for T
where
    T: BeaconChainInfo,
{
    #[inline]
    fn get_genesis_segment_root(&self) -> Result<SegmentRoot, DeriveSuperSegmentForBlockError> {
        Ok(self
            .get_segment_header(LocalSegmentIndex::ZERO)
            .ok_or(DeriveSuperSegmentForBlockError::GenesisBeaconChainSegmentHeaderNotFound)?
            .root)
    }

    #[inline]
    fn segment_roots_for_block(
        &self,
        block_number: BlockNumber,
    ) -> impl ExactSizeIterator<Item = ShardSegmentRoot> + Send + Sync + 'static {
        self.segment_headers_for_block(block_number)
            .into_iter()
            .map(|segment_header| ShardSegmentRoot {
                shard_index: ShardIndex::BEACON_CHAIN,
                segment_index: segment_header.index.as_inner(),
                segment_root: segment_header.root,
            })
    }

    #[inline(always)]
    fn previous_super_segment_header(
        &self,
        block_number: BlockNumber,
    ) -> Option<SuperSegmentHeader> {
        BeaconChainInfo::previous_super_segment_header(self, block_number)
    }

    #[inline(always)]
    fn shard_segment_roots(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Result<Arc<[ShardSegmentRoot]>, ShardSegmentRootsError> {
        BeaconChainInfo::shard_segment_roots(self, ancestor_block_number, descendant_block_root)
    }
}

/// Shard segment roots included in a base block that the blocks of its shard confirmation window
/// didn't postpone due to a shard reorg, in the order of the base block.
///
/// `shard_segment_roots` are included in the base block, `window_shard_segment_roots` in each block
/// of its shard confirmation window.
fn surviving_shard_segment_roots<'a>(
    shard_segment_roots: &'a [ShardSegmentRoot],
    window_shard_segment_roots: &[Arc<[ShardSegmentRoot]>],
) -> impl Iterator<Item = ShardSegmentRoot> + use<'a> {
    // A shard segment listed again means a shard reorg, which postpones that segment and every
    // segment of the shard above it
    let mut min_listed_again_local_segment_indices =
        HashMap::<ShardIndex, LocalSegmentIndex>::new();
    for shard_segment_root in window_shard_segment_roots
        .iter()
        .flat_map(|roots| roots.iter())
    {
        let min_local_segment_index = min_listed_again_local_segment_indices
            .entry(shard_segment_root.shard_index)
            .or_insert(shard_segment_root.segment_index);
        *min_local_segment_index = (*min_local_segment_index).min(shard_segment_root.segment_index);
    }

    // Keep anything that was not reorged
    shard_segment_roots
        .iter()
        .filter(move |shard_segment_root| {
            min_listed_again_local_segment_indices
                .get(&shard_segment_root.shard_index)
                .is_none_or(|&min_local_segment_index| {
                    shard_segment_root.segment_index < min_local_segment_index
                })
        })
        .copied()
}

/// Derive a super segment for a block with a specified parent block number and root.
///
/// For block `n` shard segment roots of blocks
/// `n - block_confirmation_depth - shard_confirmation_depth ..= n - block_confirmation_depth` on
/// its chain are read (see [`ConsensusConstants::block_confirmation_depth`] and
/// [`ConsensusConstants::shard_confirmation_depth`]).
pub fn derive_super_segment_for_block<BCI>(
    chain_info: &BCI,
    parent_block_number: BlockNumber,
    parent_block_root: &BlockRoot,
    block_confirmation_depth: BlockNumber,
    shard_confirmation_depth: BlockNumber,
) -> Result<Option<SuperSegment>, DeriveSuperSegmentForBlockError>
where
    BCI: DeriveSuperSegmentForBlockChainInfo,
{
    if parent_block_number == BlockNumber::ZERO {
        let shard_segment_root = ShardSegmentRootWithPosition {
            shard_index: ShardIndex::BEACON_CHAIN,
            segment_position: SegmentPosition::from(0),
            local_segment_index: LocalSegmentIndex::ZERO,
            segment_root: chain_info.get_genesis_segment_root()?,
        };

        let mut super_segment = SuperSegment::new(
            &SuperSegmentHeader {
                // Placeholder value will be fixed up later
                index: SuperSegmentIndex::ZERO.into(),
                root: SuperSegmentRoot::default(),
                prev_super_segment_header_hash: Blake3Hash::default(),
                // Placeholder value will be fixed up later
                max_segment_index: SegmentIndex::ZERO.into(),
                target_beacon_chain_block_number: BlockNumber::ZERO.into(),
                num_segments: 0,
            },
            BlockNumber::ONE,
            StdArc::new([shard_segment_root]),
        )
        .expect("Genesis super segment is always valid; qed");

        super_segment.header = SuperSegmentHeader {
            index: SuperSegmentIndex::ZERO.into(),
            max_segment_index: SegmentIndex::ZERO.into(),
            prev_super_segment_header_hash: Blake3Hash::default(),
            ..super_segment.header
        };

        return Ok(Some(super_segment));
    }

    let target_block_number = parent_block_number + BlockNumber::ONE;

    let own_segment_roots = chain_info.segment_roots_for_block(target_block_number);

    let shard_segment_roots_with_window = target_block_number
        .checked_sub(block_confirmation_depth.saturating_add(shard_confirmation_depth))
        .map(|base_block_number| {
            let shard_segment_roots =
                chain_info.shard_segment_roots(base_block_number, parent_block_root)?;
            let window_shard_segment_roots = (base_block_number + BlockNumber::ONE..)
                .take(u64::from(shard_confirmation_depth) as usize)
                .map(|block_number| chain_info.shard_segment_roots(block_number, parent_block_root))
                .collect::<Result<Vec<_>, _>>()?;

            Ok::<_, ShardSegmentRootsError>((shard_segment_roots, window_shard_segment_roots))
        })
        .transpose()?;
    let shard_segment_roots = shard_segment_roots_with_window.iter().flat_map(
        |(shard_segment_roots, window_shard_segment_roots)| {
            surviving_shard_segment_roots(shard_segment_roots, window_shard_segment_roots)
        },
    );

    let segment_roots = own_segment_roots
        .chain(shard_segment_roots)
        .zip(0..)
        .map(
            |(shard_segment_root, segment_position)| ShardSegmentRootWithPosition {
                shard_index: shard_segment_root.shard_index,
                segment_position: SegmentPosition::from(segment_position),
                local_segment_index: shard_segment_root.segment_index,
                segment_root: shard_segment_root.segment_root,
            },
        )
        .collect::<StdArc<_>>();

    if segment_roots.is_empty() {
        return Ok(None);
    }

    let num_segments = segment_roots.len();

    let previous_super_segment_header = chain_info
        .previous_super_segment_header(target_block_number)
        .ok_or(
            DeriveSuperSegmentForBlockError::ParentSuperSegmentHeaderNotFound {
                block_number: target_block_number,
            },
        )?;

    SuperSegment::new(
        &previous_super_segment_header,
        target_block_number,
        segment_roots,
    )
    .ok_or_else(|| {
        // TODO: While very unlikely, this is hypothetically possible and will need to be
        //  worked around in the block builder by excluding extra block headers, especially since
        //  the error will happen much later in the life cycle and it might be too late to revert
        //  once it is actually hit
        DeriveSuperSegmentForBlockError::TooManySegments {
            extra_segment_roots: num_segments - SuperSegmentRoot::MAX_SEGMENTS as usize,
        }
    })
    .map(Some)
}

/// Error for [`check_shard_segments_not_confirmed()`]
#[derive(Debug, thiserror::Error)]
pub enum CheckShardSegmentsNotConfirmedError {
    /// Shard segment is already confirmed
    #[error(
        "Segment {local_segment_index} of shard {shard_index} is already confirmed, max confirmed \
        local segment index is {max_confirmed_local_segment_index}"
    )]
    ShardSegmentAlreadyConfirmed {
        /// Shard index
        shard_index: ShardIndex,
        /// Local segment index listed in the block
        local_segment_index: LocalSegmentIndex,
        /// Highest confirmed local segment index of the shard
        max_confirmed_local_segment_index: LocalSegmentIndex,
    },
    /// Shard segment roots error
    #[error("Shard segment roots error: {error}")]
    ShardSegmentRootsError {
        /// Low-level error
        #[from]
        error: ShardSegmentRootsError,
    },
}

/// Chain info for [`check_shard_segments_not_confirmed()`].
///
/// Must have access to enough parent blocks.
pub trait CheckShardSegmentsNotConfirmedChainInfo: DeriveSuperSegmentForBlockChainInfo {
    /// Returns the highest local segment index of a shard that was included in super segments
    fn shard_max_local_segment_index(&self, shard_index: ShardIndex) -> Option<LocalSegmentIndex>;
}

impl<T> CheckShardSegmentsNotConfirmedChainInfo for T
where
    T: BeaconChainInfo,
{
    #[inline(always)]
    fn shard_max_local_segment_index(&self, shard_index: ShardIndex) -> Option<LocalSegmentIndex> {
        BeaconChainInfo::shard_max_local_segment_index(self, shard_index)
    }
}

/// Check that shard segments listed in a block with a specified number and parent are not
/// confirmed yet.
///
/// Shard segments of a base block that survive its shard confirmation window (see
/// [`ConsensusConstants::shard_confirmation_depth`]) are confirmed and will be included in a super
/// segment, a shard can't reorg them anymore. Such a segment listed again would be included in
/// super segments twice. Shard segments of base blocks whose super segments are not persisted yet
/// are read through the ancestry of `parent_block_root`, see
/// [`ConsensusConstants::block_confirmation_depth`].
///
/// `shard_segments` are segments of intermediate and leaf shards listed in the block.
pub fn check_shard_segments_not_confirmed<'a, BCI, Iter>(
    chain_info: &BCI,
    block_number: BlockNumber,
    parent_block_root: &BlockRoot,
    shard_segments: Iter,
    block_confirmation_depth: BlockNumber,
    shard_confirmation_depth: BlockNumber,
) -> Result<(), CheckShardSegmentsNotConfirmedError>
where
    BCI: CheckShardSegmentsNotConfirmedChainInfo,
    Iter: IntoIterator<Item = (ShardIndex, OwnSegments<'a>)>,
{
    let mut shard_segments = shard_segments.into_iter().peekable();

    // Nothing to check, so no blocks need to be read either
    if shard_segments.peek().is_none() {
        return Ok(());
    }

    // Nothing is confirmed yet unless the last base block whose shard confirmation window ends
    // before this block is above the genesis block, which lists no shard segments
    if block_number
        .checked_sub(shard_confirmation_depth)
        .and_then(|block_number| block_number.checked_sub(BlockNumber::ONE))
        .is_none_or(|last_confirmed_base_block_number| {
            last_confirmed_base_block_number == BlockNumber::ZERO
        })
    {
        return Ok(());
    }
    // The base block of this block's super segment, the first one whose super segment is not
    // persisted yet. For early blocks it is block 1 instead, the genesis block lists no shard
    // segments.
    let first_unpersisted_base_block_number = block_number
        .checked_sub(block_confirmation_depth)
        .and_then(|block_number| block_number.checked_sub(shard_confirmation_depth))
        .unwrap_or_default()
        .max(BlockNumber::ONE);

    // Each block is read once, as a base block and as a part of shard confirmation windows of
    // earlier base blocks
    let shard_segment_roots = (first_unpersisted_base_block_number..block_number)
        .map(|ancestor_block_number| {
            chain_info.shard_segment_roots(ancestor_block_number, parent_block_root)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shard_confirmation_window_length = usize::try_from(u64::from(shard_confirmation_depth))
        .ok()
        .and_then(|shard_confirmation_depth| shard_confirmation_depth.checked_add(1))
        .expect("At least `shard_confirmation_depth + 1` blocks were read above; qed");

    let mut max_confirmed_local_segment_indices = HashMap::<ShardIndex, LocalSegmentIndex>::new();
    for blocks_shard_segment_roots in shard_segment_roots.windows(shard_confirmation_window_length)
    {
        let (base_shard_segment_roots, window_shard_segment_roots) = blocks_shard_segment_roots
            .split_first()
            .expect("Window length is not zero; qed");

        for shard_segment_root in
            surviving_shard_segment_roots(base_shard_segment_roots, window_shard_segment_roots)
        {
            let max_local_segment_index = max_confirmed_local_segment_indices
                .entry(shard_segment_root.shard_index)
                .or_insert(shard_segment_root.segment_index);
            *max_local_segment_index =
                (*max_local_segment_index).max(shard_segment_root.segment_index);
        }
    }

    for (shard_index, own_segments) in shard_segments {
        // `None` is ordered below any local segment index
        let max_confirmed_local_segment_index =
            chain_info.shard_max_local_segment_index(shard_index).max(
                max_confirmed_local_segment_indices
                    .get(&shard_index)
                    .copied(),
            );

        // Local segment indices of own segments increase, so it is enough to check the first one
        if let Some(max_confirmed_local_segment_index) = max_confirmed_local_segment_index
            && own_segments.first_local_segment_index <= max_confirmed_local_segment_index
        {
            return Err(
                CheckShardSegmentsNotConfirmedError::ShardSegmentAlreadyConfirmed {
                    shard_index,
                    local_segment_index: own_segments.first_local_segment_index,
                    max_confirmed_local_segment_index,
                },
            );
        }
    }

    Ok(())
}
