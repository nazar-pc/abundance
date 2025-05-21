//! Data structures related to the owned version of [`Block`]

use crate::block::body::owned::{
    OwnedBeaconChainBlockBody, OwnedBeaconChainBlockBodyError, OwnedIntermediateShardBlockBody,
    OwnedIntermediateShardBlockBodyBuilder, OwnedIntermediateShardBlockBodyError,
    OwnedLeafShardBlockBody, OwnedLeafShardBlockBodyBuilder, OwnedLeafShardBlockBodyError,
    WritableBodyTransaction,
};
use crate::block::body::{BlockBody, IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    OwnedBeaconChainBlockHeader, OwnedBeaconChainBlockHeaderError,
    OwnedBeaconChainBlockHeaderUnsealed, OwnedIntermediateShardBlockHeader,
    OwnedIntermediateShardBlockHeaderError, OwnedIntermediateShardBlockHeaderUnsealed,
    OwnedLeafShardBlockHeader, OwnedLeafShardBlockHeaderUnsealed,
};
use crate::block::header::{
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderBeaconChainParameters,
    BlockHeaderConsensusInfo, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::{BeaconChainBlock, Block, IntermediateShardBlock, LeafShardBlock};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::SegmentRoot;
use alloc::vec::Vec;
use core::iter::TrustedLen;
use derive_more::From;

/// An owned version of [`BeaconChainBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlock {
    /// Block header
    pub header: OwnedBeaconChainBlockHeader,
    /// Block body
    pub body: OwnedBeaconChainBlockBody,
}

impl OwnedBeaconChainBlock {
    /// Initialize building of [`OwnedBeaconChainBlock`]
    pub fn init<'a, ISB>(
        own_segment_roots: &[SegmentRoot],
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<OwnedBeaconChainBlockBuilder, OwnedBeaconChainBlockBodyError>
    where
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedBeaconChainBlockBuilder {
            body: OwnedBeaconChainBlockBody::init(
                own_segment_roots,
                intermediate_shard_blocks,
                pot_checkpoints,
            )?,
        })
    }
}

/// Builder for [`OwnedBeaconChainBlock`]
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockBuilder {
    body: OwnedBeaconChainBlockBody,
}

impl OwnedBeaconChainBlockBuilder {
    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        consensus_parameters: BlockHeaderBeaconChainParameters<'_>,
    ) -> Result<OwnedBeaconChainBlockUnsealed, OwnedBeaconChainBlockHeaderError> {
        let body = self.body;
        let header = OwnedBeaconChainBlockHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            &body
                .body()
                .intermediate_shard_blocks
                .iter()
                .map(|block| block.header.root())
                .collect::<Vec<_>>(),
            consensus_parameters,
        )?;

        Ok(OwnedBeaconChainBlockUnsealed { body, header })
    }
}

/// Owned beacon chain block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockUnsealed {
    body: OwnedBeaconChainBlockBody,
    header: OwnedBeaconChainBlockHeaderUnsealed,
}

impl OwnedBeaconChainBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedBeaconChainBlock`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedBeaconChainBlock {
        let header = self.header.with_seal(seal);

        OwnedBeaconChainBlock {
            header,
            body: self.body,
        }
    }
}

/// An owned version of [`IntermediateShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlock {
    /// Block header
    pub header: OwnedIntermediateShardBlockHeader,
    /// Block body
    pub body: OwnedIntermediateShardBlockBody,
}

impl OwnedIntermediateShardBlock {
    /// Initialize building of [`OwnedIntermediateShardBlock`]
    pub fn init<'a, LSB>(
        own_segment_roots: &[SegmentRoot],
        leaf_shard_blocks: LSB,
    ) -> Result<OwnedIntermediateShardBlockBuilder, OwnedIntermediateShardBlockBodyError>
    where
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedIntermediateShardBlockBuilder {
            body_builder: OwnedIntermediateShardBlockBody::init(
                own_segment_roots,
                leaf_shard_blocks,
            )?,
        })
    }
}

/// Builder for [`OwnedIntermediateShardBlock`]
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockBuilder {
    body_builder: OwnedIntermediateShardBlockBodyBuilder,
}

impl OwnedIntermediateShardBlockBuilder {
    /// Add transaction to the body
    #[inline(always)]
    pub fn add_transaction<T>(
        &mut self,
        transaction: T,
    ) -> Result<(), OwnedIntermediateShardBlockBodyError>
    where
        T: WritableBodyTransaction,
    {
        self.body_builder.add_transaction(transaction)?;

        Ok(())
    }

    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> Result<OwnedIntermediateShardBlockUnsealed, OwnedIntermediateShardBlockHeaderError> {
        let body = self.body_builder.finish();
        let header = OwnedIntermediateShardBlockHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            beacon_chain_info,
            &body
                .body()
                .leaf_shard_blocks
                .iter()
                .map(|block| block.header.root())
                .collect::<Vec<_>>(),
        )?;

        Ok(OwnedIntermediateShardBlockUnsealed { body, header })
    }
}

/// Owned intermediate shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockUnsealed {
    body: OwnedIntermediateShardBlockBody,
    header: OwnedIntermediateShardBlockHeaderUnsealed,
}

impl OwnedIntermediateShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedIntermediateShardBlock`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedIntermediateShardBlock {
        let header = self.header.with_seal(seal);

        OwnedIntermediateShardBlock {
            header,
            body: self.body,
        }
    }
}

/// An owned version of [`LeafShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlock {
    /// Block header
    pub header: OwnedLeafShardBlockHeader,
    /// Block body
    pub body: OwnedLeafShardBlockBody,
}

impl OwnedLeafShardBlock {
    /// Initialize building of [`OwnedLeafShardBlock`]
    pub fn init(
        own_segment_roots: &[SegmentRoot],
    ) -> Result<OwnedLeafShardBlockBuilder, OwnedLeafShardBlockBodyError> {
        Ok(OwnedLeafShardBlockBuilder {
            body_builder: OwnedLeafShardBlockBody::init(own_segment_roots)?,
        })
    }
}

/// Builder for [`OwnedLeafShardBlock`]
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockBuilder {
    body_builder: OwnedLeafShardBlockBodyBuilder,
}

impl OwnedLeafShardBlockBuilder {
    /// Add transaction to the body
    #[inline(always)]
    pub fn add_transaction<T>(&mut self, transaction: T) -> Result<(), OwnedLeafShardBlockBodyError>
    where
        T: WritableBodyTransaction,
    {
        self.body_builder.add_transaction(transaction)?;

        Ok(())
    }

    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> OwnedLeafShardBlockUnsealed {
        let body = self.body_builder.finish();
        let header = OwnedLeafShardBlockHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            beacon_chain_info,
        );
        OwnedLeafShardBlockUnsealed { body, header }
    }
}

/// Owned leaf shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockUnsealed {
    body: OwnedLeafShardBlockBody,
    header: OwnedLeafShardBlockHeaderUnsealed,
}

impl OwnedLeafShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedLeafShardBlock`]
    pub fn with_seal(self, seal: BlockHeaderSeal<'_>) -> OwnedLeafShardBlock {
        let header = self.header.with_seal(seal);

        OwnedLeafShardBlock {
            header,
            body: self.body,
        }
    }
}

// TODO: A variant that holds both header and body in the same allocation?
/// An owned version of [`Block`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone, From)]
pub enum OwnedBlock {
    /// Block corresponds to the beacon chain
    BeaconChain(OwnedBeaconChainBlock),
    /// Block corresponds to an intermediate shard
    IntermediateShard(OwnedIntermediateShardBlock),
    /// Block corresponds to a leaf shard
    LeafShard(OwnedLeafShardBlock),
}

impl OwnedBlock {
    /// Get block header
    #[inline(always)]
    pub fn header(&self) -> BlockHeader<'_> {
        match self {
            Self::BeaconChain(block) => BlockHeader::BeaconChain(block.header.header()),
            Self::IntermediateShard(block) => BlockHeader::IntermediateShard(block.header.header()),
            Self::LeafShard(block) => BlockHeader::LeafShard(block.header.header()),
        }
    }

    /// Get block body
    #[inline(always)]
    pub fn body(&self) -> BlockBody<'_> {
        match self {
            Self::BeaconChain(block) => BlockBody::BeaconChain(block.body.body()),
            Self::IntermediateShard(block) => BlockBody::IntermediateShard(block.body.body()),
            Self::LeafShard(block) => BlockBody::LeafShard(block.body.body()),
        }
    }

    /// Get block
    #[inline(always)]
    pub fn block(&self) -> Block<'_> {
        match self {
            Self::BeaconChain(block) => Block::BeaconChain(BeaconChainBlock {
                header: block.header.header(),
                body: block.body.body(),
            }),
            Self::IntermediateShard(block) => Block::IntermediateShard(IntermediateShardBlock {
                header: block.header.header(),
                body: block.body.body(),
            }),
            Self::LeafShard(block) => Block::LeafShard(LeafShardBlock {
                header: block.header.header(),
                body: block.body.body(),
            }),
        }
    }
}
