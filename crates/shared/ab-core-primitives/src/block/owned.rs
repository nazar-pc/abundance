//! Data structures related to the owned version of [`Block`]

// Slow under Miri and these features don't change what the tests exercise, so under Miri they only
// run with just `alloc` or with all features (like when testing the whole workspace)
#[cfg(all(
    test,
    any(
        not(miri),
        not(any(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        )),
        all(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        ),
    ),
))]
mod tests;

use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedBeaconChainBody, OwnedBeaconChainBodyError, OwnedBlockBody,
    OwnedIntermediateShardBody, OwnedIntermediateShardBodyError, OwnedLeafShardBlockBodyBuilder,
    OwnedLeafShardBody, OwnedLeafShardBodyError, WritableBodyTransaction,
};
use crate::block::body::{BlockBody, IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderUnsealed,
    OwnedBlockHeader, OwnedIntermediateShardHeader, OwnedIntermediateShardHeaderUnsealed,
    OwnedLeafShardHeader, OwnedLeafShardHeaderUnsealed,
};
use crate::block::header::{
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderConsistencyError, BlockHeaderPrefix,
    BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::{BeaconChainBlock, Block, GenericBlock, IntermediateShardBlock, LeafShardBlock};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{RealShardKind, ShardIndex};
use ab_aligned_buffer::SharedAlignedBuffer;
use core::any::Any;
use core::fmt;
use core::iter::TrustedLen;
use derive_more::From;

/// Generic owned block.
///
/// Can't be implemented outside of this module, so that code generic over owned blocks can rely on
/// the header corresponding to the body, like with concrete owned blocks.
pub impl(self) trait GenericOwnedBlock:
    Clone + fmt::Debug + Send + Sync + Into<OwnedBlock> + Any + 'static
{
    /// Shard kind
    const SHARD_KIND: RealShardKind;

    /// Block header type
    type Header: GenericOwnedBlockHeader;
    /// Block body type
    type Body: GenericOwnedBlockBody;
    /// Block
    type Block<'a>: GenericBlock<'a>
    where
        Self: 'a;

    /// Split into header and body
    fn split(self) -> (Self::Header, Self::Body);

    /// Block header
    fn header(&self) -> &Self::Header;

    /// Block body
    fn body(&self) -> &Self::Body;

    /// Create owned block from buffers
    fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self>;

    /// Get regular block out of the owned version
    fn block(&self) -> Self::Block<'_>;
}

/// An owned version of [`BeaconChainBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
///
/// Fields can't be modified after creation, so that the header always corresponds to the body,
/// which constructors ensure.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlock {
    /// Block header
    pub mut(self) header: OwnedBeaconChainHeader,
    /// Block body
    pub mut(self) body: OwnedBeaconChainBody,
}

impl GenericOwnedBlock for OwnedBeaconChainBlock {
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    type Header = OwnedBeaconChainHeader;
    type Body = OwnedBeaconChainBody;
    type Block<'a> = BeaconChainBlock<'a>;

    #[inline(always)]
    fn split(self) -> (Self::Header, Self::Body) {
        (self.header, self.body)
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        &self.header
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body {
        &self.body
    }

    #[inline(always)]
    fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn block(&self) -> Self::Block<'_> {
        self.block()
    }
}

impl OwnedBeaconChainBlock {
    /// Initialize building of [`OwnedBeaconChainBlock`]
    pub fn init<'a, OS, ISB>(
        own_segments: OS,
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<OwnedBeaconChainBlockBuilder, OwnedBeaconChainBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedBeaconChainBlockBuilder {
            body: OwnedBeaconChainBody::new(
                own_segments,
                intermediate_shard_blocks,
                pot_checkpoints,
            )?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedBeaconChainHeader::from_buffer(header).ok()?,
            body: OwnedBeaconChainBody::from_buffer(body).ok()?,
        };

        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Create an owned version of a borrowed block, whose header corresponds to the body: borrowed
    /// blocks are only created by the parser with all checks or out of owned blocks
    #[inline(always)]
    pub(super) fn from_block(block: BeaconChainBlock<'_>) -> Self {
        Self {
            header: block.header.to_owned(),
            body: block.body.to_owned(),
        }
    }

    /// Get [`BeaconChainBlock`] out of [`OwnedBeaconChainBlock`]
    pub fn block(&self) -> BeaconChainBlock<'_> {
        BeaconChainBlock {
            header: self.header.header().clone(),
            body: *self.body.body(),
        }
    }
}

/// Builder for [`OwnedBeaconChainBlock`]
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockBuilder {
    body: OwnedBeaconChainBody,
}

impl OwnedBeaconChainBlockBuilder {
    /// Add header.
    ///
    /// Returns an error if the header parts are not consistent with each other or with the shard
    /// kind.
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        consensus_parameters: &BlockHeaderConsensusParameters<'_>,
    ) -> Result<OwnedBeaconChainBlockUnsealed, BlockHeaderConsistencyError> {
        let body = self.body;
        let header = OwnedBeaconChainHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            &body.body().intermediate_shard_blocks().headers_root(),
            consensus_parameters,
        )?;

        Ok(OwnedBeaconChainBlockUnsealed { body, header })
    }
}

/// Owned beacon chain block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBlockUnsealed {
    body: OwnedBeaconChainBody,
    header: OwnedBeaconChainHeaderUnsealed,
}

impl OwnedBeaconChainBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedBeaconChainBlock`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedBeaconChainBlock, BlockHeaderConsistencyError> {
        let header = self.header.with_seal(seal)?;

        Ok(OwnedBeaconChainBlock {
            header,
            body: self.body,
        })
    }
}

/// An owned version of [`IntermediateShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
///
/// Fields can't be modified after creation, so that the header always corresponds to the body,
/// which constructors ensure.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlock {
    /// Block header
    pub mut(self) header: OwnedIntermediateShardHeader,
    /// Block body
    pub mut(self) body: OwnedIntermediateShardBody,
}

impl GenericOwnedBlock for OwnedIntermediateShardBlock {
    const SHARD_KIND: RealShardKind = RealShardKind::IntermediateShard;

    type Header = OwnedIntermediateShardHeader;
    type Body = OwnedIntermediateShardBody;
    type Block<'a> = IntermediateShardBlock<'a>;

    #[inline(always)]
    fn split(self) -> (Self::Header, Self::Body) {
        (self.header, self.body)
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        &self.header
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body {
        &self.body
    }

    #[inline(always)]
    fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn block(&self) -> Self::Block<'_> {
        self.block()
    }
}

impl OwnedIntermediateShardBlock {
    /// Initialize building of [`OwnedIntermediateShardBlock`]
    pub fn init<'a, OS, LSB>(
        own_segments: OS,
        leaf_shard_blocks: LSB,
    ) -> Result<OwnedIntermediateShardBlockBuilder, OwnedIntermediateShardBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedIntermediateShardBlockBuilder {
            body: OwnedIntermediateShardBody::new(own_segments, leaf_shard_blocks)?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedIntermediateShardHeader::from_buffer(header).ok()?,
            body: OwnedIntermediateShardBody::from_buffer(body).ok()?,
        };

        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Create an owned version of a borrowed block, whose header corresponds to the body: borrowed
    /// blocks are only created by the parser with all checks or out of owned blocks
    #[inline(always)]
    pub(super) fn from_block(block: IntermediateShardBlock<'_>) -> Self {
        Self {
            header: block.header.to_owned(),
            body: block.body.to_owned(),
        }
    }

    /// Get [`IntermediateShardBlock`] out of [`OwnedIntermediateShardBlock`]
    pub fn block(&self) -> IntermediateShardBlock<'_> {
        IntermediateShardBlock {
            header: self.header.header().clone(),
            body: *self.body.body(),
        }
    }
}

/// Errors for [`OwnedIntermediateShardBlockBuilder`]
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum OwnedIntermediateShardBlockBuilderError {
    /// Header parts are not consistent with each other or with the shard kind
    #[error("Invalid header parts: {error}")]
    InvalidHeaderParts {
        /// Block header consistency error
        #[from]
        error: BlockHeaderConsistencyError,
    },
    /// Leaf shard block is not a child of the shard of the header
    #[error("Leaf shard block of shard {leaf_shard_index} is not a child of shard {shard_index}")]
    LeafShardBlockOfAnotherShard {
        /// Shard index of the header
        shard_index: ShardIndex,
        /// Shard index of the leaf shard block
        leaf_shard_index: ShardIndex,
    },
}

/// Builder for [`OwnedIntermediateShardBlock`]
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockBuilder {
    body: OwnedIntermediateShardBody,
}

impl OwnedIntermediateShardBlockBuilder {
    /// Add header.
    ///
    /// Returns an error if the header parts are not consistent with each other or with the shard
    /// kind, or if a leaf shard block is not a child of the shard of the header.
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> Result<OwnedIntermediateShardBlockUnsealed, OwnedIntermediateShardBlockBuilderError> {
        let body = self.body;
        let header = OwnedIntermediateShardHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            beacon_chain_info,
            &body.body().leaf_shard_blocks().headers_root(),
        )?;

        if let Some(leaf_shard_index) =
            IntermediateShardBlock::find_leaf_shard_block_of_another_shard(
                body.body().leaf_shard_blocks(),
                prefix.shard_index,
            )
        {
            return Err(
                OwnedIntermediateShardBlockBuilderError::LeafShardBlockOfAnotherShard {
                    shard_index: prefix.shard_index,
                    leaf_shard_index,
                },
            );
        }

        Ok(OwnedIntermediateShardBlockUnsealed { body, header })
    }
}

/// Owned intermediate shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockUnsealed {
    body: OwnedIntermediateShardBody,
    header: OwnedIntermediateShardHeaderUnsealed,
}

impl OwnedIntermediateShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedIntermediateShardBlock`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedIntermediateShardBlock, BlockHeaderConsistencyError> {
        let header = self.header.with_seal(seal)?;

        Ok(OwnedIntermediateShardBlock {
            header,
            body: self.body,
        })
    }
}

/// An owned version of [`LeafShardBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
///
/// Fields can't be modified after creation, so that the header always corresponds to the body,
/// which constructors ensure.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlock {
    /// Block header
    pub mut(self) header: OwnedLeafShardHeader,
    /// Block body
    pub mut(self) body: OwnedLeafShardBody,
}

impl GenericOwnedBlock for OwnedLeafShardBlock {
    const SHARD_KIND: RealShardKind = RealShardKind::LeafShard;

    type Header = OwnedLeafShardHeader;
    type Body = OwnedLeafShardBody;
    type Block<'a> = LeafShardBlock<'a>;

    #[inline(always)]
    fn split(self) -> (Self::Header, Self::Body) {
        (self.header, self.body)
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        &self.header
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body {
        &self.body
    }

    #[inline(always)]
    fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn block(&self) -> Self::Block<'_> {
        self.block()
    }
}

impl OwnedLeafShardBlock {
    /// Initialize building of [`OwnedLeafShardBlock`]
    pub fn init<OS>(own_segments: OS) -> Result<OwnedLeafShardBlockBuilder, OwnedLeafShardBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
    {
        Ok(OwnedLeafShardBlockBuilder {
            body_builder: OwnedLeafShardBody::init(own_segments)?,
        })
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self> {
        let block = Self {
            header: OwnedLeafShardHeader::from_buffer(header).ok()?,
            body: OwnedLeafShardBody::from_buffer(body).ok()?,
        };

        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
    }

    /// Create an owned version of a borrowed block, whose header corresponds to the body: borrowed
    /// blocks are only created by the parser with all checks or out of owned blocks
    #[inline(always)]
    pub(super) fn from_block(block: LeafShardBlock<'_>) -> Self {
        Self {
            header: block.header.to_owned(),
            body: block.body.to_owned(),
        }
    }

    /// Get [`LeafShardBlock`] out of [`OwnedLeafShardBlock`]
    pub fn block(&self) -> LeafShardBlock<'_> {
        LeafShardBlock {
            header: self.header.header().clone(),
            body: *self.body.body(),
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

    /// Add header.
    ///
    /// Returns an error if the header parts are not consistent with each other or with the shard
    /// kind.
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> Result<OwnedLeafShardBlockUnsealed, BlockHeaderConsistencyError> {
        let body = self.body_builder.finish();
        let header = OwnedLeafShardHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body.body().root(),
                state_root,
            },
            consensus_info,
            beacon_chain_info,
        )?;
        Ok(OwnedLeafShardBlockUnsealed { body, header })
    }
}

/// Owned leaf shard block header, which is not sealed yet
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockUnsealed {
    body: OwnedLeafShardBody,
    header: OwnedLeafShardHeaderUnsealed,
}

impl OwnedLeafShardBlockUnsealed {
    /// Hash of the block before seal is applied to it
    #[inline(always)]
    pub fn pre_seal_hash(&self) -> Blake3Hash {
        self.header.pre_seal_hash()
    }

    /// Add seal and return [`OwnedLeafShardBlock`].
    ///
    /// Returns an error if the seal doesn't correspond to the public key hash of the solution.
    pub fn with_seal(
        self,
        seal: BlockHeaderSeal<'_>,
    ) -> Result<OwnedLeafShardBlock, BlockHeaderConsistencyError> {
        let header = self.header.with_seal(seal)?;

        Ok(OwnedLeafShardBlock {
            header,
            body: self.body,
        })
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
    /// Split into header and body
    #[inline]
    pub fn split(self) -> (OwnedBlockHeader, OwnedBlockBody) {
        match self {
            Self::BeaconChain(block) => {
                let (header, body) = block.split();
                (header.into(), body.into())
            }
            Self::IntermediateShard(block) => {
                let (header, body) = block.split();
                (header.into(), body.into())
            }
            Self::LeafShard(block) => {
                let (header, body) = block.split();
                (header.into(), body.into())
            }
        }
    }

    /// Block header
    #[inline(always)]
    pub fn header(&self) -> BlockHeader<'_> {
        match self {
            Self::BeaconChain(block) => BlockHeader::BeaconChain(block.header.header().clone()),
            Self::IntermediateShard(block) => {
                BlockHeader::IntermediateShard(block.header.header().clone())
            }
            Self::LeafShard(block) => BlockHeader::LeafShard(block.header.header().clone()),
        }
    }

    /// Block body
    #[inline(always)]
    pub fn body(&self) -> BlockBody<'_> {
        match self {
            Self::BeaconChain(block) => BlockBody::BeaconChain(*block.body.body()),
            Self::IntermediateShard(block) => BlockBody::IntermediateShard(*block.body.body()),
            Self::LeafShard(block) => BlockBody::LeafShard(*block.body.body()),
        }
    }

    /// Create owned block from buffers
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
        shard_kind: RealShardKind,
    ) -> Option<Self> {
        Some(match shard_kind {
            RealShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainBlock::from_buffers(header, body)?)
            }
            RealShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardBlock::from_buffers(header, body)?)
            }
            RealShardKind::LeafShard => {
                Self::LeafShard(OwnedLeafShardBlock::from_buffers(header, body)?)
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
