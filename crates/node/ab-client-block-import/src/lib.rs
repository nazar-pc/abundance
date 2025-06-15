#![feature(array_windows)]

pub mod beacon_chain;

use ab_client_api::BlockOrigin;
use ab_core_primitives::block::BlockRoot;

/// Error for [`BlockImport`]
#[derive(Debug, thiserror::Error)]
pub enum BlockImportError {
    /// Unknown parent block
    #[error("Unknown parent block: {block_root}")]
    UnknownParentBlock {
        // Block root that was not found
        block_root: BlockRoot,
    },
    /// Custom import error
    #[error("Custom import error: {error}")]
    Custom {
        // Custom block import error
        #[from]
        error: anyhow::Error,
    },
}

/// Block import interface
pub trait BlockImport<Block>: Send + Sync {
    /// Import provided block
    ///
    /// Parent block must either be imported already or at least queued for import. Block import is
    /// immediately added to the queue, but actual import may not happen unless the returned future
    /// is polled.
    fn import(
        &self,
        // TODO: Some way to attack state storage items
        block: Block,
        origin: BlockOrigin,
    ) -> Result<impl Future<Output = Result<(), BlockImportError>> + Send, BlockImportError>;
}
