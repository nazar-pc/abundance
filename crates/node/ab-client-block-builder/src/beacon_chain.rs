//! Block building for the beacon chain

use crate::{BlockBuilder, BlockBuilderError};
use ab_client_api::ChainInfo;
use ab_client_archiving::segment_headers_store::SegmentHeadersStore;
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersError, derive_consensus_parameters,
};
use ab_core_primitives::block::body::owned::OwnedBeaconChainBodyError;
use ab_core_primitives::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, BlockHeaderPrefix,
    OwnedBlockHeaderConsensusParameters, OwnedBlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, SlotNumber};
use ab_core_primitives::shard::ShardIndex;
use std::iter;
use std::time::SystemTime;

/// Error for [`BeaconChainBlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BeaconChainBlockBuilderError {
    /// Consensus parameters derivation error
    #[error("Consensus parameters derivation error: {error}")]
    ConsensusParametersDerivation {
        /// Consensus parameters derivation error
        #[from]
        error: DeriveConsensusParametersError,
    },
    /// Failed to create body
    #[error("Failed to create body: {error}")]
    FailedToCreateBody {
        // Body creation error
        #[from]
        error: OwnedBeaconChainBodyError,
    },
    /// Failed to create a header
    #[error("Failed to create a header: {error}")]
    FailedToCreateHeader {
        // Header creation error
        #[from]
        error: OwnedBeaconChainHeaderError,
    },
}

impl From<BeaconChainBlockBuilderError> for BlockBuilderError {
    #[inline(always)]
    fn from(error: BeaconChainBlockBuilderError) -> Self {
        Self::Custom {
            error: error.into(),
        }
    }
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

        let header_prefix = self.create_header_prefix(
            parent_block_root,
            parent_header.prefix.timestamp,
            block_number,
        );
        let consensus_parameters = self.derive_consensus_parameters(
            parent_block_root,
            parent_header,
            block_number,
            consensus_info.slot,
        )?;

        let block_builder = OwnedBeaconChainBlock::init(
            self.segment_headers_store
                .segment_headers_for_block(block_number)
                .into_iter()
                .map(|segment_header| segment_header.segment_root),
            // TODO: Real intermediate shard blocks
            iter::empty(),
            checkpoints,
        )
        .map_err(BeaconChainBlockBuilderError::from)?;

        let block_unsealed = block_builder
            .with_header(
                &header_prefix,
                // TODO: Real state root
                Default::default(),
                consensus_info,
                consensus_parameters.as_ref(),
            )
            .map_err(BeaconChainBlockBuilderError::from)?;

        let seal = seal_block(block_unsealed.pre_seal_hash())
            .await
            .ok_or(BlockBuilderError::FailedToSeal)?;
        let block = block_unsealed.with_seal(seal.as_ref());

        Ok(block)
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
        parent_timestamp: BlockTimestamp,
        block_number: BlockNumber,
    ) -> BlockHeaderPrefix {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let mut timestamp = BlockTimestamp::new(u64::try_from(timestamp).unwrap_or(u64::MAX));

        if timestamp <= parent_timestamp {
            timestamp = BlockTimestamp::new(parent_timestamp.as_ms().saturating_add(1));
        }

        BlockHeaderPrefix {
            number: block_number,
            shard_index: ShardIndex::BEACON_CHAIN,
            padding_0: [0; _],
            timestamp,
            parent_root: *parent_block_root,
            // TODO: Real MMR root
            mmr_root: Default::default(),
        }
    }

    fn derive_consensus_parameters(
        &self,
        parent_block_root: &BlockRoot,
        parent_header: &BeaconChainHeader<'_>,
        block_number: BlockNumber,
        slot: SlotNumber,
    ) -> Result<OwnedBlockHeaderConsensusParameters, BeaconChainBlockBuilderError> {
        let derived_consensus_parameters = derive_consensus_parameters(
            &self.consensus_constants,
            &self.chain_info,
            parent_block_root,
            parent_header.consensus_parameters(),
            parent_header.consensus_info.slot,
            block_number,
            slot,
        )?;

        Ok(OwnedBlockHeaderConsensusParameters {
            fixed_parameters: derived_consensus_parameters.fixed_parameters,
            // TODO: Super segment support
            super_segment_root: None,
            next_solution_range: derived_consensus_parameters.next_solution_range,
            pot_parameters_change: derived_consensus_parameters.pot_parameters_change,
        })
    }
}
