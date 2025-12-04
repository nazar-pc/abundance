//! Data structures related to the owned version of [`Block`]

use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedBeaconChainBody, OwnedBeaconChainBodyError, OwnedBlockBody,
    OwnedIntermediateShardBody, OwnedIntermediateShardBodyError, OwnedLeafShardBlockBodyBuilder,
    OwnedLeafShardBody, OwnedLeafShardBodyError, WritableBodyTransaction,
};
use crate::block::body::{BlockBody, IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedBeaconChainHeaderUnsealed, OwnedBlockHeader, OwnedIntermediateShardHeader,
    OwnedIntermediateShardHeaderError, OwnedIntermediateShardHeaderUnsealed, OwnedLeafShardHeader,
    OwnedLeafShardHeaderUnsealed,
};
use crate::block::header::{
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::{BeaconChainBlock, Block, GenericBlock, IntermediateShardBlock, LeafShardBlock};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::SegmentRoot;
use crate::shard::RealShardKind;
use ab_aligned_buffer::SharedAlignedBuffer;
use alloc::vec::Vec;
use core::fmt;
use core::iter::TrustedLen;
use derive_more::From;

/// Generic owned block
pub trait GenericOwnedBlock: Clone + fmt::Debug + Send + Sync + Into<OwnedBlock> + 'static {
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

    // TODO: Unchecked versions of methods that create instances from buffers (here and in
    //  header/block)?
    /// Create owned block from buffers
    fn from_buffers(header: SharedAlignedBuffer, body: SharedAlignedBuffer) -> Option<Self>;

    /// Get regular block out of the owned version
    fn block(&self) -> Self::Block<'_>;
}

/// An owned version of [`BeaconChainBlock`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct OwnedBeaconChainBlock {
    /// Block header
    pub header: OwnedBeaconChainHeader,
    /// Block body
    pub body: OwnedBeaconChainBody,
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
    pub fn init<'a, OSR, ISB>(
        own_segment_roots: OSR,
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<OwnedBeaconChainBlockBuilder, OwnedBeaconChainBodyError>
    where
        OSR: TrustedLen<Item = SegmentRoot>,
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedBeaconChainBlockBuilder {
            body: OwnedBeaconChainBody::new(
                own_segment_roots,
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

        // TODO: This duplicates parsing done in above constructors
        if !block.block().is_internally_consistent() {
            return None;
        }

        Some(block)
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
    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        consensus_parameters: BlockHeaderConsensusParameters<'_>,
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
                .intermediate_shard_blocks()
                .iter()
                .map(|block| *block.header.root())
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
    header: OwnedBeaconChainHeaderUnsealed,
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
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct OwnedIntermediateShardBlock {
    /// Block header
    pub header: OwnedIntermediateShardHeader,
    /// Block body
    pub body: OwnedIntermediateShardBody,
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
    pub fn init<'a, OSR, LSB>(
        own_segment_roots: OSR,
        leaf_shard_blocks: LSB,
    ) -> Result<OwnedIntermediateShardBlockBuilder, OwnedIntermediateShardBodyError>
    where
        OSR: TrustedLen<Item = SegmentRoot>,
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
    {
        Ok(OwnedIntermediateShardBlockBuilder {
            body: OwnedIntermediateShardBody::new(own_segment_roots, leaf_shard_blocks)?,
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
            header: self.header.header().clone(),
            body: *self.body.body(),
        }
    }
}

/// Builder for [`OwnedIntermediateShardBlock`]
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockBuilder {
    body: OwnedIntermediateShardBody,
}

impl OwnedIntermediateShardBlockBuilder {
    /// Add header
    pub fn with_header(
        self,
        prefix: &BlockHeaderPrefix,
        state_root: Blake3Hash,
        consensus_info: &BlockHeaderConsensusInfo,
        beacon_chain_info: &BlockHeaderBeaconChainInfo,
    ) -> Result<OwnedIntermediateShardBlockUnsealed, OwnedIntermediateShardHeaderError> {
        let body = self.body;
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
                .leaf_shard_blocks()
                .iter()
                .map(|block| *block.header.root())
                .collect::<Vec<_>>(),
        )?;

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
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct OwnedLeafShardBlock {
    /// Block header
    pub header: OwnedLeafShardHeader,
    /// Block body
    pub body: OwnedLeafShardBody,
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
    pub fn init<OSR>(
        own_segment_roots: OSR,
    ) -> Result<OwnedLeafShardBlockBuilder, OwnedLeafShardBodyError>
    where
        OSR: TrustedLen<Item = SegmentRoot>,
    {
        Ok(OwnedLeafShardBlockBuilder {
            body_builder: OwnedLeafShardBody::init(own_segment_roots)?,
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
    header: OwnedLeafShardHeaderUnsealed,
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

    // TODO: Unchecked versions of methods that create instances from buffers (here and in
    //  header/block)?
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
