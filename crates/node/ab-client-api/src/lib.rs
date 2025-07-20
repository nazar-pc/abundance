//! Client API

#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]

use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_merkle_tree::mmr::MerkleMountainRange;
use rclite::Arc;

// TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that allows the
//  code to compile. Constant 4294967295 is hardcoded here and below for compilation to succeed.
const _: () = {
    assert!(u32::MAX == 4294967295);
};

/// Type alias for Merkle Mountain Range with block roots.
///
/// NOTE: `u32` is smaller than `BlockNumber`'s internal `u64` but will be sufficient for a long
/// time and substantially decrease the size of the data structure.
pub type BlockMerkleMountainRange = MerkleMountainRange<4294967295>;

// TODO: Probably move it elsewhere
/// Origin
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BlockOrigin {
    /// Created locally
    Local,
    /// Received during the sync process
    Sync,
    /// Broadcast on the network during normal operation (not sync)
    Broadcast,
}

/// Error for [`ChainInfoWrite::persist_block()`]
#[derive(Debug, thiserror::Error)]
pub enum PersistBlockError {
    // TODO
}

// TODO: Split this into different more narrow traits
/// Chain info
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

    /// Get header of ancestor block number for descendant block root
    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<Block::Header>;

    /// Block header
    fn header(&self, block_root: &BlockRoot) -> Option<Block::Header>;

    /// Merkle Mountain Range with block
    fn mmr_with_block(&self, block_root: &BlockRoot) -> Option<Arc<BlockMerkleMountainRange>>;
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
        mmr_with_block: Arc<BlockMerkleMountainRange>,
    ) -> impl Future<Output = Result<(), PersistBlockError>> + Send;
}

/// Chain sync status
pub trait ChainSyncStatus: Clone + Send + Sync + 'static {
    /// Block number that the sync process is targeting right now.
    ///
    /// Can be zero if not syncing actively.
    fn target_block_number(&self) -> BlockNumber;

    /// Returns `true` if the chain is currently syncing
    fn is_syncing(&self) -> bool;

    /// Returns `true` if the node is currently offline
    fn is_offline(&self) -> bool;
}
