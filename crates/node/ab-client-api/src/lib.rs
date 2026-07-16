//! Client API

#![feature(const_convert, const_trait_impl)]

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBeaconChainBlock};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{
    LocalSegmentIndex, SegmentHeader, SegmentIndex, SegmentRoot, SuperSegmentHeader,
    SuperSegmentIndex,
};
use ab_core_primitives::shard::ShardIndex;
use ab_merkle_tree::mmr::MerkleMountainRange;
use rclite::Arc;
use std::io;
use std::sync::Arc as StdArc;

const MAX_U32_AS_U64: u64 = u64::from(u32::MAX);
/// Type alias for Merkle Mountain Range with block roots.
///
/// NOTE: `u32` is smaller than `BlockNumber`'s internal `u64` but will be sufficient for a long
/// time and substantially decrease the size of the in-memory data structure.
pub type BlockMerkleMountainRange = MerkleMountainRange<MAX_U32_AS_U64>;

/// State of a contract slot
#[derive(Debug, Clone)]
pub struct ContractSlotState {
    /// Owner of the slot
    pub owner: Address,
    /// Contract that manages the slot
    pub contract: Address,
    /// Slot contents
    pub contents: SharedAlignedBuffer,
}

/// Additional details about a block
#[derive(Debug, Clone)]
pub struct BlockDetails {
    /// Merkle Mountain Range with block
    pub mmr_with_block: Arc<BlockMerkleMountainRange>,
    /// System contracts state after block
    pub system_contract_states: StdArc<[ContractSlotState]>,
}

// TODO: Probably move it elsewhere
/// Origin
#[derive(Debug, Clone)]
pub enum BlockOrigin {
    // TODO: Take advantage of this in block import
    /// Created locally by block builder
    LocalBlockBuilder {
        /// Additional details about a block
        block_details: BlockDetails,
    },
    /// Received during the sync process
    Sync,
    /// Broadcast on the network during normal operation (not sync)
    Broadcast,
}

/// Intermediate or leaf shard segment root information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardSegmentRoot {
    /// Shard index
    pub shard_index: ShardIndex,
    /// Local segment index
    pub segment_index: LocalSegmentIndex,
    /// Segment root
    pub segment_root: SegmentRoot,
}

/// Error for [`ChainInfo::block()`]
#[derive(Debug, thiserror::Error)]
pub enum ReadBlockError {
    /// Unknown block root
    #[error("Unknown block root")]
    UnknownBlockRoot,
    /// Failed to decode the block
    #[error("Failed to decode the block")]
    FailedToDecode,
    /// Storage item read error
    #[error("Storage item read error")]
    StorageItemReadError {
        /// Low-level error
        #[from]
        error: io::Error,
    },
}

/// Error for [`ChainInfoWrite::persist_block()`]
#[derive(Debug, thiserror::Error)]
pub enum PersistBlockError {
    /// Missing parent
    #[error("Missing parent")]
    MissingParent,
    /// Block is outside the acceptable range
    #[error("Block is outside the acceptable range")]
    OutsideAcceptableRange,
    /// Storage item write error
    #[error("Storage item write error")]
    StorageItemWriteError {
        /// Low-level error
        #[from]
        error: io::Error,
    },
}

/// Error for [`ChainInfoWrite::persist_segment_headers()`]
#[derive(Debug, thiserror::Error)]
pub enum PersistSegmentHeadersError {
    /// Segment index must strictly follow the last segment index, can't store segment header
    #[error(
        "Segment index {local_segment_index} must strictly follow last segment index \
        {last_local_segment_index}, can't store segment header"
    )]
    MustFollowLastSegmentIndex {
        /// Segment index that was attempted to be inserted
        local_segment_index: LocalSegmentIndex,
        /// Last segment index
        last_local_segment_index: LocalSegmentIndex,
    },
    /// The first segment index must be zero
    #[error("First segment index must be zero, found {local_segment_index}")]
    FirstSegmentIndexZero {
        /// Segment index that was attempted to be inserted
        local_segment_index: LocalSegmentIndex,
    },
    /// Storage item write error
    #[error("Storage item write error")]
    StorageItemWriteError {
        /// Low-level error
        #[from]
        error: io::Error,
    },
}

/// Error for [`BeaconChainInfo::shard_segment_roots()`]
#[derive(Debug, thiserror::Error)]
pub enum ShardSegmentRootsError {
    /// Block missing
    #[error("Block {block_number} is missing")]
    BlockMissing {
        /// The block number that is missing in the database
        block_number: BlockNumber,
    },
}

/// Error for [`BeaconChainInfoWrite::persist_super_segment_headers()`]
#[derive(Debug, thiserror::Error)]
pub enum PersistSuperSegmentHeadersError {
    /// Super segment index must strictly follow the last super segment index, can't store super
    /// segment header
    #[error(
        "Super segment index {super_segment_index} must strictly follow last super segment index \
        {last_super_segment_index}, can't store super segment header"
    )]
    MustFollowLastSegmentIndex {
        /// Super segment index that was attempted to be inserted
        super_segment_index: SuperSegmentIndex,
        /// Last super segment index
        last_super_segment_index: SuperSegmentIndex,
    },
    /// The first super segment index must be zero
    #[error("First super segment index must be zero, found {super_segment_index}")]
    FirstSegmentIndexZero {
        /// Super segment index that was attempted to be inserted
        super_segment_index: SuperSegmentIndex,
    },
    /// Storage item write error
    #[error("Storage item write error")]
    StorageItemWriteError {
        /// Low-level error
        #[from]
        error: io::Error,
    },
}

// TODO: Split this into different more narrow traits
/// Chain info.
///
/// NOTE:
/// <div class="warning">
/// Blocks or their parts returned from these APIs are reference-counted and cheap to clone.
/// However, it is not expected that they will be retained in memory for a long time. Blocks and
/// headers will not be pruned until their reference count goes down to one. This is imported when
/// there is an ongoing block import happening and its parent must exist until the import
/// finishes.
/// </div>
pub trait ChainInfo<Block>: Clone + Send + Sync + 'static
where
    Block: GenericOwnedBlock,
{
    /// Best block root
    fn best_root(&self) -> BlockRoot;

    // TODO: Uncomment if/when necessary
    // /// Find root of ancestor block number for descendant block root
    // fn ancestor_root(
    //     &self,
    //     ancestor_block_number: BlockNumber,
    //     descendant_block_root: &BlockRoot,
    // ) -> Option<BlockRoot>;

    /// Best block header
    fn best_header(&self) -> Block::Header;

    /// Returns the best block header like [`Self::best_header()`] with additional block details
    fn best_header_with_details(&self) -> (Block::Header, BlockDetails);

    /// Get header of ancestor block number for descendant block root
    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<Block::Header>;

    /// Block header
    fn header(&self, block_root: &BlockRoot) -> Option<Block::Header>;

    /// Returns a block header like [`Self::header()`] with additional block details
    fn header_with_details(&self, block_root: &BlockRoot) -> Option<(Block::Header, BlockDetails)>;

    fn block(
        &self,
        block_root: &BlockRoot,
    ) -> impl Future<Output = Result<Block, ReadBlockError>> + Send;

    /// Returns the last observed local segment header of this shard
    fn last_segment_header(&self) -> Option<SegmentHeader>;

    /// Get a single segment header
    fn get_segment_header(&self, segment_index: LocalSegmentIndex) -> Option<SegmentHeader>;

    /// Get segment headers that are expected to be included at specified block number
    fn segment_headers_for_block(&self, block_number: BlockNumber) -> Vec<SegmentHeader>;
}

/// [`ChainInfo`] extension for writing information
pub trait ChainInfoWrite<Block>: ChainInfo<Block>
where
    Block: GenericOwnedBlock,
{
    /// Persist newly imported block
    fn persist_block(
        &self,
        block: Block,
        block_details: BlockDetails,
    ) -> impl Future<Output = Result<(), PersistBlockError>> + Send;

    /// Persist segment headers.
    ///
    /// Multiple can be inserted for efficiency purposes.
    fn persist_segment_headers(
        &self,
        segment_headers: Vec<SegmentHeader>,
    ) -> impl Future<Output = Result<(), PersistSegmentHeadersError>> + Send;
}

/// Beacon chain info
pub trait BeaconChainInfo: ChainInfo<OwnedBeaconChainBlock> {
    /// Returns intermediate and leaf shard segment roots included in the specified block number.
    ///
    /// NOTE: Since blocks at this depth are already confirmed, only a block number is needed as a
    /// reference.
    fn shard_segment_roots(
        &self,
        block_number: BlockNumber,
    ) -> Result<StdArc<[ShardSegmentRoot]>, ShardSegmentRootsError>;

    /// Returns the last observed super segment header
    fn last_super_segment_header(&self) -> Option<SuperSegmentHeader>;

    /// Returns the previous super segment header for the block built with the specified target
    /// block number.
    ///
    /// `None` is returned for blocks <= 1.
    fn previous_super_segment_header(
        &self,
        block_number: BlockNumber,
    ) -> Option<SuperSegmentHeader>;

    /// Get a single super segment header
    fn get_super_segment_header(
        &self,
        super_segment_index: SuperSegmentIndex,
    ) -> Option<SuperSegmentHeader>;

    /// Get a single super segment header for a segment index
    fn get_super_segment_header_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> Option<SuperSegmentHeader>;
}

/// [`BeaconChainInfo`] extension for writing information
pub trait BeaconChainInfoWrite: BeaconChainInfo + ChainInfoWrite<OwnedBeaconChainBlock> {
    /// Persist a new super segment header.
    ///
    /// Returns `Ok(true)` if the header was inserted, `Ok(false)` if it was already present.
    #[must_use]
    fn persist_super_segment_header(
        &self,
        super_segment_header: SuperSegmentHeader,
    ) -> impl Future<Output = Result<bool, PersistSuperSegmentHeadersError>> + Send;

    /// Persist super segment headers.
    ///
    /// Multiple can be inserted for efficiency purposes.
    fn persist_super_segment_headers(
        &self,
        super_segment_headers: Vec<SuperSegmentHeader>,
    ) -> impl Future<Output = Result<(), PersistSuperSegmentHeadersError>> + Send;
}

/// Chain sync status
pub trait ChainSyncStatus: Clone + Send + Sync + 'static {
    /// The block number that the sync process is targeting right now.
    ///
    /// Can be zero if not syncing actively.
    fn target_block_number(&self) -> BlockNumber;

    /// Returns `true` if the chain is currently syncing
    fn is_syncing(&self) -> bool;

    /// Returns `true` if the node is currently offline
    fn is_offline(&self) -> bool;
}
