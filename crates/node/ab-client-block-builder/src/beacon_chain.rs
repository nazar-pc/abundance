use crate::{BlockBuilder, BlockBuilderError};
use ab_client_api::ChainInfo;
use ab_client_archiving::segment_headers_store::SegmentHeadersStore;
use ab_client_consensus_common::ConsensusConstants;
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersError, derive_consensus_parameters,
};
use ab_core_primitives::block::body::owned::{OwnedBeaconChainBody, OwnedBeaconChainBodyError};
use ab_core_primitives::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedBeaconChainHeaderUnsealed,
};
use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderPrefix,
    BlockHeaderResult, OwnedBlockHeaderConsensusParameters, OwnedBlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, SlotNumber};
use ab_core_primitives::segments::SegmentRoot;
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
    /// Failed to create header
    #[error("Failed to create header: {error}")]
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

        let header_prefix = self.create_header_prefix(parent_block_root, block_number);
        let consensus_parameters = self.derive_consensus_parameters(
            parent_block_root,
            parent_header,
            block_number,
            consensus_info.slot,
        )?;

        let own_segment_roots = self.own_segment_roots(block_number);

        let body = self.create_body(&own_segment_roots, checkpoints)?;
        let header_unsealed = self.create_header_unsealed(
            &header_prefix,
            consensus_info,
            consensus_parameters.as_ref(),
            &BlockHeaderResult {
                body_root: body.body().root(),
                // TODO: Real state root
                state_root: Default::default(),
            },
        )?;
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

    fn own_segment_roots(&self, block_number: BlockNumber) -> Vec<SegmentRoot> {
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
        let derived_consensus_parameters = derive_consensus_parameters(
            &self.consensus_constants,
            &self.chain_info,
            parent_block_root,
            &parent_header.consensus_parameters,
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
