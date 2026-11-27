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
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsistencyError,
    BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal, OwnedBlockHeaderConsensusParameters,
};
use crate::block::{
    BeaconChainBlock, Block, GenericBlock, IntermediateShardBlock, LeafShardBlock,
    align_to_and_ensure_zero_padding,
};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{RealShardKind, ShardIndex};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align16;
use ab_io_type::alignment::Alignment;
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

    /// Create owned block from a header and a body.
    ///
    /// Returns them back if the header doesn't correspond to the body.
    fn from_parts(
        header: Self::Header,
        body: Self::Body,
    ) -> Result<Self, (Self::Header, Self::Body)>;

    /// Create owned block from buffers.
    ///
    /// Returns them back if they don't contain a header and a body of a block.
    fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)>;

    /// Create owned block from a buffer with a whole block, see [`Block::try_from_bytes()`] for the
    /// layout.
    ///
    /// Headers are usually kept in memory much longer than bodies, so the header is copied into a
    /// separate allocation, while the body references `buffer`. Returns `buffer` back if it doesn't
    /// contain exactly a block.
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer>;

    /// Get regular block out of the owned version
    fn block(&self) -> Self::Block<'_>;
}

/// Split the buffer of a whole block into its header and the buffer of its body, see
/// [`Block::try_from_bytes()`] for the layout.
///
/// Headers are usually kept in memory much longer than bodies, so the beginning of the buffer, at
/// most `max_header_size` bytes, is copied into a separate allocation for the header, while the
/// body references `buffer`. Returns `None` if `buffer` doesn't contain a header followed by
/// padding and a non-empty body.
fn split_block_buffer<Header, HeaderFromBufferStart>(
    buffer: &SharedAlignedBuffer,
    max_header_size: u32,
    header_from_buffer_start: HeaderFromBufferStart,
) -> Option<(Header, SharedAlignedBuffer)>
where
    Header: GenericOwnedBlockHeader,
    HeaderFromBufferStart: FnOnce(SharedAlignedBuffer) -> Result<Header, SharedAlignedBuffer>,
{
    let header_bytes = buffer
        .get_slice(..buffer.len().min(max_header_size))
        .expect("Not longer than the buffer; qed");
    let header =
        header_from_buffer_start(SharedAlignedBuffer::from_bytes(header_bytes.as_slice())).ok()?;

    let bytes = buffer.as_slice();
    let after_header = bytes
        .get(header.buffer().as_slice().len()..)
        .expect("Header is a part of the buffer; qed");
    let body_bytes = align_to_and_ensure_zero_padding(after_header, Alignment::MAX)?;
    if body_bytes.is_empty() {
        return None;
    }

    let body_offset = u32::try_from(bytes.len() - body_bytes.len())
        .expect("Body is a part of the buffer, whose length is `u32`; qed");
    let body = buffer
        .get_slice(body_offset..)
        .expect("Body is a part of the buffer; qed")
        .try_into_start_align::<Align16>()
        .expect("Body follows padding to `Alignment::MAX` in memory; qed");

    Some((header, body))
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
    fn from_parts(
        header: Self::Header,
        body: Self::Body,
    ) -> Result<Self, (Self::Header, Self::Body)> {
        Self::from_parts(header, body)
    }

    #[inline(always)]
    fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
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
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + 'a,
    {
        Ok(OwnedBeaconChainBlockBuilder {
            body: OwnedBeaconChainBody::new(
                own_segments,
                intermediate_shard_blocks,
                pot_checkpoints,
            )?,
        })
    }

    /// Create owned block from a header and a body.
    ///
    /// Returns them back if the header doesn't correspond to the body.
    #[inline]
    pub fn from_parts(
        header: OwnedBeaconChainHeader,
        body: OwnedBeaconChainBody,
    ) -> Result<Self, (OwnedBeaconChainHeader, OwnedBeaconChainBody)> {
        let block = Self { header, body };

        if !block.block().is_internally_consistent() {
            return Err((block.header, block.body));
        }

        Ok(block)
    }

    /// Create owned block from buffers.
    ///
    /// Returns them back if they don't contain a header and a body of a block.
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        let header = match OwnedBeaconChainHeader::from_buffer(header) {
            Ok(header) => header,
            Err(header) => return Err((header, body)),
        };
        let body = match OwnedBeaconChainBody::from_buffer(body) {
            Ok(body) => body,
            Err(body) => return Err((header.buffer().clone(), body)),
        };

        Self::from_parts(header, body)
            .map_err(|(header, body)| (header.buffer().clone(), body.buffer().clone()))
    }

    /// Create owned block from a buffer with a whole block, see
    /// [`GenericOwnedBlock::from_buffer()`]
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((header, body)) = split_block_buffer(
            &buffer,
            OwnedBeaconChainHeader::MAX_ALLOCATION,
            OwnedBeaconChainHeader::from_buffer_start,
        ) else {
            return Err(buffer);
        };
        let Ok(body) = OwnedBeaconChainBody::from_buffer(body) else {
            return Err(buffer);
        };

        Self::from_parts(header, body).map_err(|_parts| buffer)
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
        consensus_parameters: &OwnedBlockHeaderConsensusParameters,
    ) -> Result<OwnedBeaconChainBlockUnsealed, BlockHeaderConsistencyError> {
        let body = self.body;
        // Header roots of intermediate shard blocks are expensive to compute, so they are only
        // computed once for both the header field and the body root
        let intermediate_shard_blocks_root = body.body().intermediate_shard_blocks().headers_root();
        let header = OwnedBeaconChainHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body
                    .body()
                    .root_with_intermediate_shard_blocks_root(&intermediate_shard_blocks_root),
                state_root,
            },
            consensus_info,
            &intermediate_shard_blocks_root,
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
    fn from_parts(
        header: Self::Header,
        body: Self::Body,
    ) -> Result<Self, (Self::Header, Self::Body)> {
        Self::from_parts(header, body)
    }

    #[inline(always)]
    fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
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
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + 'a,
    {
        Ok(OwnedIntermediateShardBlockBuilder {
            body: OwnedIntermediateShardBody::new(own_segments, leaf_shard_blocks)?,
        })
    }

    /// Create owned block from a header and a body.
    ///
    /// Returns them back if the header doesn't correspond to the body.
    #[inline]
    pub fn from_parts(
        header: OwnedIntermediateShardHeader,
        body: OwnedIntermediateShardBody,
    ) -> Result<Self, (OwnedIntermediateShardHeader, OwnedIntermediateShardBody)> {
        let block = Self { header, body };

        if !block.block().is_internally_consistent() {
            return Err((block.header, block.body));
        }

        Ok(block)
    }

    /// Create owned block from buffers.
    ///
    /// Returns them back if they don't contain a header and a body of a block.
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        let header = match OwnedIntermediateShardHeader::from_buffer(header) {
            Ok(header) => header,
            Err(header) => return Err((header, body)),
        };
        let body = match OwnedIntermediateShardBody::from_buffer(body) {
            Ok(body) => body,
            Err(body) => return Err((header.buffer().clone(), body)),
        };

        Self::from_parts(header, body)
            .map_err(|(header, body)| (header.buffer().clone(), body.buffer().clone()))
    }

    /// Create owned block from a buffer with a whole block, see
    /// [`GenericOwnedBlock::from_buffer()`]
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((header, body)) = split_block_buffer(
            &buffer,
            OwnedIntermediateShardHeader::MAX_ALLOCATION,
            OwnedIntermediateShardHeader::from_buffer_start,
        ) else {
            return Err(buffer);
        };
        let Ok(body) = OwnedIntermediateShardBody::from_buffer(body) else {
            return Err(buffer);
        };

        Self::from_parts(header, body).map_err(|_parts| buffer)
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
        // Header roots of leaf shard blocks are expensive to compute, so they are only computed
        // once for both the header field and the body root
        let leaf_shard_blocks_root = body.body().leaf_shard_blocks().headers_root();
        let header = OwnedIntermediateShardHeader::from_parts(
            prefix,
            &BlockHeaderResult {
                body_root: body
                    .body()
                    .root_with_leaf_shard_blocks_root(&leaf_shard_blocks_root),
                state_root,
            },
            consensus_info,
            beacon_chain_info,
            &leaf_shard_blocks_root,
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
    fn from_parts(
        header: Self::Header,
        body: Self::Body,
    ) -> Result<Self, (Self::Header, Self::Body)> {
        Self::from_parts(header, body)
    }

    #[inline(always)]
    fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        Self::from_buffers(header, body)
    }

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
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

    /// Create owned block from a header and a body.
    ///
    /// Returns them back if the header doesn't correspond to the body.
    #[inline]
    pub fn from_parts(
        header: OwnedLeafShardHeader,
        body: OwnedLeafShardBody,
    ) -> Result<Self, (OwnedLeafShardHeader, OwnedLeafShardBody)> {
        let block = Self { header, body };

        if !block.block().is_internally_consistent() {
            return Err((block.header, block.body));
        }

        Ok(block)
    }

    /// Create owned block from buffers.
    ///
    /// Returns them back if they don't contain a header and a body of a block.
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        let header = match OwnedLeafShardHeader::from_buffer(header) {
            Ok(header) => header,
            Err(header) => return Err((header, body)),
        };
        let body = match OwnedLeafShardBody::from_buffer(body) {
            Ok(body) => body,
            Err(body) => return Err((header.buffer().clone(), body)),
        };

        Self::from_parts(header, body)
            .map_err(|(header, body)| (header.buffer().clone(), body.buffer().clone()))
    }

    /// Create owned block from a buffer with a whole block, see
    /// [`GenericOwnedBlock::from_buffer()`]
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some((header, body)) = split_block_buffer(
            &buffer,
            OwnedLeafShardHeader::MAX_ALLOCATION,
            OwnedLeafShardHeader::from_buffer_start,
        ) else {
            return Err(buffer);
        };
        let Ok(body) = OwnedLeafShardBody::from_buffer(body) else {
            return Err(buffer);
        };

        Self::from_parts(header, body).map_err(|_parts| buffer)
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
                body_root: body.root(),
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

    /// Create owned block from buffers.
    ///
    /// The shard kind of the block is that of the shard index in the prefix of its header, see
    /// [`BlockHeader::shard_kind_from_bytes()`]. Returns buffers back if they don't contain a
    /// header and a body of a block.
    #[inline]
    pub fn from_buffers(
        header: SharedAlignedBuffer,
        body: SharedAlignedBuffer,
    ) -> Result<Self, (SharedAlignedBuffer, SharedAlignedBuffer)> {
        let Some(shard_kind) = BlockHeader::shard_kind_from_bytes(header.as_slice()) else {
            return Err((header, body));
        };

        Ok(match shard_kind {
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

    /// Create owned block from a buffer with a whole block, see
    /// [`GenericOwnedBlock::from_buffer()`].
    ///
    /// The shard kind of the block is that of the shard index in the prefix of its header, see
    /// [`BlockHeader::shard_kind_from_bytes()`].
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        let Some(shard_kind) = BlockHeader::shard_kind_from_bytes(buffer.as_slice()) else {
            return Err(buffer);
        };

        Ok(match shard_kind {
            RealShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainBlock::from_buffer(buffer)?)
            }
            RealShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardBlock::from_buffer(buffer)?)
            }
            RealShardKind::LeafShard => Self::LeafShard(OwnedLeafShardBlock::from_buffer(buffer)?),
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
