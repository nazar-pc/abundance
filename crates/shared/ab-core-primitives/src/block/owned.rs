//! Data structures related to the owned version of [`Block`]

use crate::block::body::owned::{
    OwnedBeaconChainBody, OwnedBeaconChainBodyError, OwnedIntermediateShardBlockBodyBuilder,
    OwnedIntermediateShardBody, OwnedIntermediateShardBodyError, OwnedLeafShardBlockBodyBuilder,
    OwnedLeafShardBody, OwnedLeafShardBodyError, WritableBodyTransaction,
};
use crate::block::body::{BlockBody, IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    OwnedBeaconChainBlockHeaderUnsealed, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedIntermediateShardBlockHeaderUnsealed, OwnedIntermediateShardHeader,
    OwnedIntermediateShardHeaderError, OwnedLeafShardBlockHeaderUnsealed, OwnedLeafShardHeader,
};
use crate::block::header::{
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderBeaconChainParameters,
    BlockHeaderConsensusInfo, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSealRef,
};
use crate::block::{BeaconChainBlock, Block, IntermediateShardBlock, LeafShardBlock};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::SegmentRoot;
use crate::shard::ShardKind;
use ab_aligned_buffer::SharedAlignedBuffer;
use alloc::vec::Vec;
use core::iter::TrustedLen;
use derive_more::From;

/// Errors for [`OwnedBeaconChainBlock`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBeaconChainBlockError {
    /// Beacon chain block header error
    #[error("Beacon chain block header error: {0}")]
    Header(#[from] OwnedBeaconChainHeaderError),
    /// Beacon chain block body error
    #[error("Beacon chain block body error: {0}")]
    Body(#[from] OwnedBeaconChainBodyError),
}

/// An owned version of [`BeaconChainBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlock {
    /// Block header
    pub header: OwnedBeaconChainHeader,
    /// Block body
    pub body: OwnedBeaconChainBody,
}

impl OwnedBeaconChainBlock {
    /// Initialize building of [`OwnedBeaconChainBlock`]
    pub fn init<'a, ISB>(
        own_segment_roots: &[SegmentRoot],
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<OwnedBeaconChainBlockBuilder, OwnedBeaconChainBodyError>
    where
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedBeaconChainBlockBuilder {
            body: OwnedBeaconChainBody::init(
                own_segment_roots,
                intermediate_shard_blocks,
                pot_checkpoints,
            )?,
        })
    }

    /// Create owned block from a reference
    #[inline]
    pub fn from_block(block: BeaconChainBlock<'_>) -> Result<Self, OwnedBeaconChainBlockError> {
        Ok(Self {
            header: OwnedBeaconChainHeader::from_header(block.header)?,
            body: OwnedBeaconChainBody::from_body(block.body)?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedBeaconChainHeader::from_buffer(header).ok()?,
            body: OwnedBeaconChainBody::from_buffer(body).ok()?,
        };

        // TODO: This duplicates parsing done in above constructors
        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Get [`BeaconChainBlock`] out of [`OwnedBeaconChainBlock`]
    pub fn block(&self) -> BeaconChainBlock<'_> {
        BeaconChainBlock {
            header: self.header.header(),
            body: self.body.body(),
        }
    }
}

/// Builder for [`OwnedBeaconChainBlock`]
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockBuilder {
    body: OwnedBeaconChainBody,
}

impl OwnedBeaconChainBlockBuilder {
    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        consensus_parameters: BlockHeaderBeaconChainParameters<'_>,
    ) -> Result<OwnedBeaconChainBlockUnsealed, OwnedBeaconChainHeaderError> {
        let body = self.body;
        let header = OwnedBeaconChainHeader::from_parts(
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
    body: OwnedBeaconChainBody,
    header: OwnedBeaconChainBlockHeaderUnsealed,
}

impl OwnedBeaconChainBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedBeaconChainBlock`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedBeaconChainBlock {
        let header = self.header.with_seal(seal);

        OwnedBeaconChainBlock {
            header,
            body: self.body,
        }
    }
}

/// Errors for [`OwnedIntermediateShardBlock`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardBlockError {
    /// Intermediate shard block header error
    #[error("Intermediate shard block header error: {0}")]
    Header(#[from] OwnedIntermediateShardHeaderError),
    /// Intermediate shard block body error
    #[error("Intermediate shard block body error: {0}")]
    Body(#[from] OwnedIntermediateShardBodyError),
}

/// An owned version of [`IntermediateShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlock {
    /// Block header
    pub header: OwnedIntermediateShardHeader,
    /// Block body
    pub body: OwnedIntermediateShardBody,
}

impl OwnedIntermediateShardBlock {
    /// Initialize building of [`OwnedIntermediateShardBlock`]
    pub fn init<'a, LSB>(
        own_segment_roots: &[SegmentRoot],
        leaf_shard_blocks: LSB,
    ) -> Result<OwnedIntermediateShardBlockBuilder, OwnedIntermediateShardBodyError>
    where
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedIntermediateShardBlockBuilder {
            body_builder: OwnedIntermediateShardBody::init(own_segment_roots, leaf_shard_blocks)?,
        })
    }

    /// Create owned block from a reference
    #[inline]
    pub fn from_block(
        block: IntermediateShardBlock<'_>,
    ) -> Result<Self, OwnedIntermediateShardBlockError> {
        Ok(Self {
            header: OwnedIntermediateShardHeader::from_header(block.header)?,
            body: OwnedIntermediateShardBody::from_body(block.body)?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedIntermediateShardHeader::from_buffer(header).ok()?,
            body: OwnedIntermediateShardBody::from_buffer(body).ok()?,
        };

        // TODO: This duplicates parsing done in above constructors
        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Get [`IntermediateShardBlock`] out of [`OwnedIntermediateShardBlock`]
    pub fn block(&self) -> IntermediateShardBlock<'_> {
        IntermediateShardBlock {
            header: self.header.header(),
            body: self.body.body(),
        }
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
    ) -> Result<(), OwnedIntermediateShardBodyError>
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
    ) -> Result<OwnedIntermediateShardBlockUnsealed, OwnedIntermediateShardHeaderError> {
        let body = self.body_builder.finish();
        let header = OwnedIntermediateShardHeader::from_parts(
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
    body: OwnedIntermediateShardBody,
    header: OwnedIntermediateShardBlockHeaderUnsealed,
}

impl OwnedIntermediateShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedIntermediateShardBlock`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedIntermediateShardBlock {
        let header = self.header.with_seal(seal);

        OwnedIntermediateShardBlock {
            header,
            body: self.body,
        }
    }
}

/// Errors for [`OwnedLeafShardBlock`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedLeafShardBlockError {
    /// Leaf shard block body error
    #[error("Leaf shard block body error: {0}")]
    Body(#[from] OwnedLeafShardBodyError),
}

/// An owned version of [`LeafShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlock {
    /// Block header
    pub header: OwnedLeafShardHeader,
    /// Block body
    pub body: OwnedLeafShardBody,
}

impl OwnedLeafShardBlock {
    /// Initialize building of [`OwnedLeafShardBlock`]
    pub fn init(
        own_segment_roots: &[SegmentRoot],
    ) -> Result<OwnedLeafShardBlockBuilder, OwnedLeafShardBodyError> {
        Ok(OwnedLeafShardBlockBuilder {
            body_builder: OwnedLeafShardBody::init(own_segment_roots)?,
        })
    }

    /// Create owned block from a reference
    #[inline]
    pub fn from_block(block: LeafShardBlock<'_>) -> Result<Self, OwnedLeafShardBlockError> {
        Ok(Self {
            header: OwnedLeafShardHeader::from_header(block.header),
            body: OwnedLeafShardBody::from_body(block.body)?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedLeafShardHeader::from_buffer(header).ok()?,
            body: OwnedLeafShardBody::from_buffer(body).ok()?,
        };

        // TODO: This duplicates parsing done in above constructors
        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Get [`LeafShardBlock`] out of [`OwnedLeafShardBlock`]
    pub fn block(&self) -> LeafShardBlock<'_> {
        LeafShardBlock {
            header: self.header.header(),
            body: self.body.body(),
        }
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
    pub fn add_transaction<T>(&mut self, transaction: T) -> Result<(), OwnedLeafShardBodyError>
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
        let header = OwnedLeafShardHeader::from_parts(
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
    body: OwnedLeafShardBody,
    header: OwnedLeafShardBlockHeaderUnsealed,
}

impl OwnedLeafShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedLeafShardBlock`]
    pub fn with_seal(self, seal: BlockHeaderSealRef<'_>) -> OwnedLeafShardBlock {
        let header = self.header.with_seal(seal);

        OwnedLeafShardBlock {
            header,
            body: self.body,
        }
    }
}

/// Errors for [`OwnedBlock`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBlockError {
    /// Beacon chain block error
    #[error("Beacon chain block error: {0}")]
    BeaconChain(#[from] OwnedBeaconChainBlockError),
    /// Intermediate shard block error
    #[error("Intermediate shard block error: {0}")]
    IntermediateShard(#[from] OwnedIntermediateShardBlockError),
    /// Leaf shard block error
    #[error("Leaf shard block error: {0}")]
    LeafShard(#[from] OwnedLeafShardBlockError),
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

    /// Create owned block from a reference
    #[inline]
    pub fn from_block(block: Block<'_>) -> Result<Self, OwnedBlockError> {
        Ok(match block {
            Block::BeaconChain(block) => {
                Self::BeaconChain(OwnedBeaconChainBlock::from_block(block)?)
            }
            Block::IntermediateShard(block) => {
                Self::IntermediateShard(OwnedIntermediateShardBlock::from_block(block)?)
            }
            Block::LeafShard(block) => Self::LeafShard(OwnedLeafShardBlock::from_block(block)?),
        })
    }

    // TODO: Unchecked versions of methods that create instances from buffers (here and in
    //  header/block)?
    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
        shard_kind: ShardKind,
    ) -> Option<Self> {
        Some(match shard_kind {
            ShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainBlock::from_buffers(header, body)?)
            }
            ShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardBlock::from_buffers(header, body)?)
            }
            ShardKind::LeafShard => {
                Self::LeafShard(OwnedLeafShardBlock::from_buffers(header, body)?)
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                return None;
            }
        })
    }

    /// Get block
    #[inline(always)]
    pub fn block(&self) -> Block<'_> {
        match self {
            Self::BeaconChain(block) => Block::BeaconChain(block.block()),
            Self::IntermediateShard(block) => Block::IntermediateShard(block.block()),
            Self::LeafShard(block) => Block::LeafShard(block.block()),
        }
    }
}
