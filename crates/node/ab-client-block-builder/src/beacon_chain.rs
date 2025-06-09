use crate::{BlockBuilder, BlockBuilderError, ConsensusConstants};
use ab_client_block_import::segment_headers_store::SegmentHeadersStore;
use ab_core_primitives::block::body::owned::{OwnedBeaconChainBody, OwnedBeaconChainBodyError};
use ab_core_primitives::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedBeaconChainHeaderUnsealed,
};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderResult,
    OwnedBlockHeaderConsensusParameters, OwnedBlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::PotCheckpoints;
use ab_core_primitives::segments::SegmentRoot;
use ab_core_primitives::shard::ShardIndex;
use std::iter;
use std::time::SystemTime;

/// Error for [`BeaconChainBlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BeaconChainBlockBuilderError {
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
#[derive(Debug)]
pub struct BeaconChainBlockBuilder {
    segment_headers_store: SegmentHeadersStore,
    consensus_constants: ConsensusConstants,
}

impl BlockBuilder<OwnedBeaconChainBlock> for BeaconChainBlockBuilder {
    fn build<SealBlock, SealBlockFut>(
        &mut self,
        parent_block_root: &BlockRoot,
        parent_header: &<OwnedBeaconChainHeader as GenericOwnedBlockHeader>::Header<'_>,
        consensus_info: &BlockHeaderConsensusInfo,
        checkpoints: &[PotCheckpoints],
        seal_block: SealBlock,
    ) -> impl Future<Output = Result<OwnedBeaconChainBlock, BlockBuilderError>> + Send
    where
        SealBlock: FnOnce(Blake3Hash) -> SealBlockFut + Send,
        SealBlockFut: Future<Output = Option<OwnedBlockHeaderSeal>> + Send,
    {
        async move {
            let block_number = parent_header.prefix.number.saturating_add(BlockNumber::ONE);

            let header_prefix = self.create_header_prefix(parent_block_root, block_number);
            let consensus_parameters =
                self.derive_consensus_parameters(block_number, parent_header);

            let own_segment_roots = self.own_segment_header_roots(block_number);

            let body = self
                .create_body(&own_segment_roots, checkpoints)
                .map_err(anyhow::Error::from)?;
            let header_unsealed = self
                .create_header_unsealed(
                    &header_prefix,
                    consensus_info,
                    consensus_parameters.as_ref(),
                )
                .map_err(anyhow::Error::from)?;
            let seal = seal_block(header_unsealed.pre_seal_hash())
                .await
                .ok_or(BlockBuilderError::FailedToSeal)?;
            let header = header_unsealed.with_seal(seal.as_ref());

            Ok(OwnedBeaconChainBlock { header, body })
        }
    }
}

impl BeaconChainBlockBuilder {
    /// Create a new instance
    pub fn new(
        segment_headers_store: SegmentHeadersStore,
        consensus_constants: ConsensusConstants,
    ) -> Self {
        Self {
            segment_headers_store,
            consensus_constants,
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
    ) -> Result<OwnedBeaconChainHeaderUnsealed, BeaconChainBlockBuilderError> {
        Ok(OwnedBeaconChainHeader::from_parts(
            prefix,
            // TODO
            &BlockHeaderResult {
                body_root: Default::default(),
                state_root: Default::default(),
            },
            consensus_info,
            // TODO: Real child shard blocks
            &[],
            consensus_parameters,
        )?)
    }

    fn derive_consensus_parameters(
        &self,
        block_number: BlockNumber,
        parent_header: &BeaconChainHeader<'_>,
    ) -> OwnedBlockHeaderConsensusParameters {
        let parent_consensus_parameters = parent_header.consensus_parameters;
        let (solution_range, next_solution_range) =
            if let Some(next_solution_range) = parent_consensus_parameters.next_solution_range {
                (next_solution_range, None)
            } else {
                let solution_range = parent_consensus_parameters.fixed_parameters.solution_range;

                if block_number % self.era_duration == Zero::zero() {
                    SolutionRanges::<T>::mutate(|solution_ranges| {
                        let next_solution_range;
                        // Check if the solution range should be adjusted for next era.
                        if !ShouldAdjustSolutionRange::<T>::get() {
                            next_solution_range = solution_ranges.current;
                        } else if let Some(solution_range_override) =
                            NextSolutionRangeOverride::<T>::take()
                        {
                            next_solution_range = solution_range_override.solution_range;
                        } else {
                            next_solution_range = solution_ranges.current.derive_next(
                                // If Era start slot is not found it means we have just finished the first era
                                EraStartSlot::<T>::get().unwrap_or_default(),
                                current_slot,
                                slot_probability,
                                BlockNumber::new(
                                    <BlockNumberFor<T> as TryInto<u64>>::try_into(era_duration)
                                        .unwrap_or_else(|_| {
                                            panic!("Era duration is always within u64; qed")
                                        }),
                                ),
                            );
                        };
                        solution_ranges.next.replace(next_solution_range);
                    });

                    EraStartSlot::<T>::put(current_slot);
                }

                (solution_range, todo!())
            };

        // TODO: Proper values here
        OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range,
                slot_iterations: todo!(),
            },
            // TODO: Segment root support
            super_segment_root: None,
            next_solution_range,
            pot_parameters_change: todo!(),
        }
    }
}
