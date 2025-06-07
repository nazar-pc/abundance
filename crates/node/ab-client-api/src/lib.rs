//! Client API

/// Chain sync status
pub trait ChainSyncStatus: Clone + Send + Sync + 'static {
    /// Returns `true` if the chain is currently syncing
    fn is_syncing(&self) -> bool;

    /// Returns `true` if the node is currently offline
    fn is_offline(&self) -> bool;
}
