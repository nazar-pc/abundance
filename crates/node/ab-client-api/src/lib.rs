//! Client API

use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::owned::GenericOwnedBlock;

/// Error for [`ChainInfo::header()`]
#[derive(Debug, thiserror::Error)]
pub enum ChainInfoHeaderError {
    // TODO: Error variants
}

// TODO: Split this into different more narrow traits
/// Chain info
pub trait ChainInfo<Block>: Clone + Send + Sync + 'static
where
    Block: GenericOwnedBlock,
{
    /// Best block root
    fn best_root(&self) -> BlockRoot;

    /// Bst block header
    fn best_header(&self) -> Block::Header;

    /// Block header
    fn header(
        &self,
        block_root: &BlockRoot,
    ) -> impl Future<Output = Result<Option<Block::Header>, ChainInfoHeaderError>> + Send;
}

/// Chain sync status
pub trait ChainSyncStatus: Clone + Send + Sync + 'static {
    /// Returns `true` if the chain is currently syncing
    fn is_syncing(&self) -> bool;

    /// Returns `true` if the node is currently offline
    fn is_offline(&self) -> bool;
}
