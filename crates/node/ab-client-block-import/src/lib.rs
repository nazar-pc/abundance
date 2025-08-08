#![feature(map_try_insert)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]

pub mod beacon_chain;
mod importing_blocks;

use ab_client_api::{BlockOrigin, PersistBlockError};
use ab_core_primitives::block::BlockRoot;

/// Error for [`BlockImport`]
#[derive(Debug, thiserror::Error)]
pub enum BlockImportError {
    /// Already importing
    #[error("Already importing")]
    AlreadyImporting,
    /// Already importing
    #[error("Already imported")]
    AlreadyImported,
    /// Unknown parent block
    #[error("Unknown parent block: {block_root}")]
    UnknownParentBlock {
        // Block root that was not found
        block_root: BlockRoot,
    },
    // TODO: Use or remove
    // /// Parent block details are missing; this is an implementation bug and must never happen
    // #[error(
    //     "Parent block details are missing; this is an implementation bug and must never happen"
    // )]
    // ParentBlockDetailsMissing,
    /// Invalid parent MMR; this is an implementation bug and must never happen
    #[error("Invalid parent MMR; this is an implementation bug and must never happen")]
    ParentBlockMmrInvalid,
    /// Can't extend MMR, too many blocks; this is an implementation bug and must never happen
    #[error(
        "Can't extend MMR, too many blocks; this is an implementation bug and must never happen"
    )]
    CantExtendMmr,
    /// Parent block import failed
    #[error("Parent block import failed")]
    ParentBlockImportFailed,
    /// Block persisting error
    #[error("Block persisting error: {error}")]
    PersistBlockError {
        /// Block persisting error
        #[from]
        error: PersistBlockError,
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
    /// Import provided block.
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
