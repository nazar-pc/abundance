//! Client API

use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};

// TODO: Probably move it elsewhere
/// Origin
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BlockOrigin {
    /// Created locally
    Local,
    /// Received during sync process
    Sync,
    /// Broadcast on the network during normal operation (not sync)
    Broadcast,
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
