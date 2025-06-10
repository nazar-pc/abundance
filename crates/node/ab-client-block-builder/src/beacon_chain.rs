use crate::{BlockBuilder, BlockBuilderError, ConsensusConstants};
use ab_client_api::ChainInfo;
use ab_client_block_import::segment_headers_store::SegmentHeadersStore;
use ab_core_primitives::block::body::owned::{OwnedBeaconChainBody, OwnedBeaconChainBodyError};
use ab_core_primitives::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedBeaconChainHeaderUnsealed,
};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderFixedConsensusParameters, BlockHeaderPotParametersChange, BlockHeaderPrefix,
    BlockHeaderResult, OwnedBlockHeaderConsensusParameters, OwnedBlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotParametersChange, SlotNumber};
use ab_core_primitives::segments::SegmentRoot;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::solutions::SolutionRange;
use std::iter;
use std::num::NonZeroU32;
use std::time::SystemTime;

/// Error for [`BeaconChainBlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BeaconChainBlockBuilderError {
    /// Failed to get ancestor header
    #[error("Failed to get ancestor header")]
    GetAncestorHeader,
    /// Failed to create body
    #[error("Failed to create body: {error}")]
    FailedToCreateBody {
        // Body creation error
        #[from]
        error: OwnedBeaconChainBodyError,
    },
    /// Failed to create header
    #[error("Failed to create header: {error}")]
    FailedToCreateHeader {
        // Header creation error
        #[from]
        error: OwnedBeaconChainHeaderError,
    },
}

struct SolutionRanges {
    current: SolutionRange,
    next: Option<SolutionRange>,
}

struct PotInfo {
    slot_iterations: NonZeroU32,
    parameters_change: Option<PotParametersChange>,
}

// TODO: Another domain-specific abstraction over `ChainInfo`, which will be implemented for
//  `ChainInfo`, but could also be implemented in simpler way directly for tests without dealing
//  with complete headers, etc.
/// Beacon chain block builder
#[derive(Debug)]
pub struct BeaconChainBlockBuilder<CI> {
    segment_headers_store: SegmentHeadersStore,
    consensus_constants: ConsensusConstants,
    chain_info: CI,
}

impl<CI> BlockBuilder<OwnedBeaconChainBlock> for BeaconChainBlockBuilder<CI>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    async fn build<SealBlock, SealBlockFut>(
        &mut self,
        parent_block_root: &BlockRoot,
        parent_header: &<OwnedBeaconChainHeader as GenericOwnedBlockHeader>::Header<'_>,
        consensus_info: &BlockHeaderConsensusInfo,
        checkpoints: &[PotCheckpoints],
        seal_block: SealBlock,
    ) -> Result<OwnedBeaconChainBlock, BlockBuilderError>
    where
        SealBlock: FnOnce(Blake3Hash) -> SealBlockFut + Send,
        SealBlockFut: Future<Output = Option<OwnedBlockHeaderSeal>> + Send,
    {
        let block_number = parent_header.prefix.number.saturating_add(BlockNumber::ONE);

        let header_prefix = self.create_header_prefix(parent_block_root, block_number);
        let consensus_parameters = self
            .derive_consensus_parameters(
                parent_block_root,
                parent_header,
                block_number,
                consensus_info.slot,
            )
            .map_err(anyhow::Error::from)?;

        let own_segment_roots = self.own_segment_header_roots(block_number);

        let body = self
            .create_body(&own_segment_roots, checkpoints)
            .map_err(anyhow::Error::from)?;
        let header_unsealed = self
            .create_header_unsealed(
                &header_prefix,
                consensus_info,
                consensus_parameters.as_ref(),
                &BlockHeaderResult {
                    body_root: body.body().root(),
                    // TODO: Real state root
                    state_root: Default::default(),
                },
            )
            .map_err(anyhow::Error::from)?;
        let seal = seal_block(header_unsealed.pre_seal_hash())
            .await
            .ok_or(BlockBuilderError::FailedToSeal)?;
        let header = header_unsealed.with_seal(seal.as_ref());

        Ok(OwnedBeaconChainBlock { header, body })
    }
}

impl<CI> BeaconChainBlockBuilder<CI>
where
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    /// Create a new instance
    pub fn new(
        segment_headers_store: SegmentHeadersStore,
        consensus_constants: ConsensusConstants,
        chain_info: CI,
    ) -> Self {
        Self {
            segment_headers_store,
            consensus_constants,
            chain_info,
        }
    }

    fn create_header_prefix(
        &self,
        parent_block_root: &BlockRoot,
        block_number: BlockNumber,
    ) -> BlockHeaderPrefix {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let timestamp = u64::try_from(timestamp).unwrap_or(u64::MAX);

        BlockHeaderPrefix {
            version: BlockHeaderPrefix::BLOCK_VERSION,
            number: block_number,
            shard_index: ShardIndex::BEACON_CHAIN,
            padding: [0; _],
            timestamp,
            parent_root: *parent_block_root,
            // TODO: Real MMR root
            mmr_root: Default::default(),
        }
    }

    fn own_segment_header_roots(&self, block_number: BlockNumber) -> Vec<SegmentRoot> {
        self.segment_headers_store
            .segment_headers_for_block(block_number)
            .into_iter()
            .map(|segment_header| segment_header.segment_root)
            .collect::<Vec<_>>()
    }

    fn create_body(
        &mut self,
        own_segment_roots: &[SegmentRoot],
        checkpoints: &[PotCheckpoints],
    ) -> Result<OwnedBeaconChainBody, BeaconChainBlockBuilderError> {
        Ok(OwnedBeaconChainBody::new(
            own_segment_roots,
            // TODO: Real intermediate shard blocks
            iter::empty(),
            checkpoints,
        )?)
    }

    fn create_header_unsealed(
        &mut self,
        prefix: &BlockHeaderPrefix,
        consensus_info: &BlockHeaderConsensusInfo,
        consensus_parameters: BlockHeaderConsensusParameters<'_>,
        result: &BlockHeaderResult,
    ) -> Result<OwnedBeaconChainHeaderUnsealed, BeaconChainBlockBuilderError> {
        Ok(OwnedBeaconChainHeader::from_parts(
            prefix,
            result,
            consensus_info,
            // TODO: Real child shard blocks
            &[],
            consensus_parameters,
        )?)
    }

    fn derive_consensus_parameters(
        &self,
        parent_block_root: &BlockRoot,
        parent_header: &BeaconChainHeader<'_>,
        block_number: BlockNumber,
        slot: SlotNumber,
    ) -> Result<OwnedBlockHeaderConsensusParameters, BeaconChainBlockBuilderError> {
        let parent_consensus_parameters = parent_header.consensus_parameters;
        let solution_ranges = self.derive_solution_ranges(
            parent_block_root,
            parent_consensus_parameters.fixed_parameters.solution_range,
            parent_consensus_parameters.next_solution_range,
            block_number,
            slot,
        )?;
        let pot_info = self.derive_pot_info(
            parent_block_root,
            parent_header.consensus_info.slot,
            parent_header
                .consensus_parameters
                .fixed_parameters
                .slot_iterations,
            parent_header
                .consensus_parameters
                .pot_parameters_change
                .copied()
                .map(PotParametersChange::from),
            block_number,
            slot,
        )?;

        Ok(OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: solution_ranges.current,
                slot_iterations: pot_info.slot_iterations,
            },
            // TODO: Segment root support
            super_segment_root: None,
            next_solution_range: solution_ranges.next,
            pot_parameters_change: pot_info
                .parameters_change
                .map(BlockHeaderPotParametersChange::from),
        })
    }

    fn derive_solution_ranges(
        &self,
        parent_block_root: &BlockRoot,
        solution_range: SolutionRange,
        next_solution_range: Option<SolutionRange>,
        block_number: BlockNumber,
        slot: SlotNumber,
    ) -> Result<SolutionRanges, BeaconChainBlockBuilderError> {
        let era_duration = self.consensus_constants.era_duration;
        let slot_probability = self.consensus_constants.slot_probability;

        if let Some(next_solution_range) = next_solution_range {
            return Ok(SolutionRanges {
                current: next_solution_range,
                next: None,
            });
        }

        let next_solution_range =
            if block_number.as_u64() % era_duration.as_u64() == 0 && block_number > era_duration {
                let era_start_block = block_number.saturating_sub(era_duration);
                let era_start_slot = self
                    .chain_info
                    .ancestor_header(era_start_block, parent_block_root)
                    .ok_or(BeaconChainBlockBuilderError::GetAncestorHeader)?
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

    fn derive_pot_info(
        &self,
        parent_block_root: &BlockRoot,
        parent_slot: SlotNumber,
        parent_slot_iterations: NonZeroU32,
        parent_parameters_change: Option<PotParametersChange>,
        block_number: BlockNumber,
        slot: SlotNumber,
    ) -> Result<PotInfo, BeaconChainBlockBuilderError> {
        let pot_entropy_injection_interval =
            self.consensus_constants.pot_entropy_injection_interval;
        let pot_entropy_injection_lookback_depth = self
            .consensus_constants
            .pot_entropy_injection_lookback_depth;
        let pot_entropy_injection_delay = self.consensus_constants.pot_entropy_injection_delay;

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
            // Retain previous PoT parameters change if it applies after block's slot
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
                    let entropy_source_block_header = self
                        .chain_info
                        .ancestor_header(entropy_source_block_number, parent_block_root)
                        .ok_or(BeaconChainBlockBuilderError::GetAncestorHeader)?;
                    let entropy_source_block_header = entropy_source_block_header.header();

                    entropy_source_block_header
                        .consensus_info
                        .proof_of_time
                        .derive_pot_entropy(
                            &entropy_source_block_header.consensus_info.solution.chunk,
                        )
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
}
