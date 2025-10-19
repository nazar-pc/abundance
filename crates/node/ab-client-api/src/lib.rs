//! Client API

#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_merkle_tree::mmr::MerkleMountainRange;
use rclite::Arc;
use std::io;
use std::sync::Arc as StdArc;

// TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that allows the
//  code to compile. Constant 4294967295 is hardcoded here and below for compilation to succeed.
#[expect(clippy::assertions_on_constants, reason = "Intentional documentation")]
#[expect(clippy::eq_op, reason = "Intentional documentation")]
const _: () = {
    assert!(u32::MAX == 4294967295);
};

// TODO: Make this a `#[transparent]` struct to improve usability (avoiding the need for
//  `generic_const_exprs` feature in downstream crates)?
/// Type alias for Merkle Mountain Range with block roots.
///
/// NOTE: `u32` is smaller than `BlockNumber`'s internal `u64` but will be sufficient for a long
/// time and substantially decrease the size of the data structure.
pub type BlockMerkleMountainRange = MerkleMountainRange<4294967295>;

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
pub trait ChainInfo<Block>: Clone + Send + Sync
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
