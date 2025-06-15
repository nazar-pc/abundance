#![feature(array_windows)]

pub mod beacon_chain;

use ab_client_api::BlockOrigin;
use ab_core_primitives::block::body::owned::GenericOwnedBlockBody;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::segments::SegmentRoot;

type GenericHeader<'a, Block> =
    <<Block as GenericOwnedBlock>::Header as GenericOwnedBlockHeader>::Header<'a>;
type GenericBody<'a, Block> =
    <<Block as GenericOwnedBlock>::Body as GenericOwnedBlockBody>::Body<'a>;

/// Error for [`BlockVerification`]
#[derive(Debug, thiserror::Error)]
pub enum BlockVerificationError {
    /// Block is below archiving point
    #[error("Block is below archiving point")]
    BelowArchivingPoint,
    /// Invalid header prefix
    #[error("Invalid heder prefix")]
    InvalidHeaderPrefix,
    /// Invalid seal
    #[error("Invalid seal")]
    InvalidSeal,
    /// Invalid own segment roots
    #[error("Invalid own segment roots")]
    InvalidOwnSegmentRoots {
        /// Expected segment roots (correct)
        expected: Vec<SegmentRoot>,
        /// Actual segment roots (invalid)
        actual: Vec<SegmentRoot>,
    },
    /// Custom verification error
    #[error("Custom verification error: {error}")]
    Custom {
        // Custom block verification error
        #[from]
        error: anyhow::Error,
    },
}

/// Block verification interface
pub trait BlockVerification<Block>: Send + Sync
where
    Block: GenericOwnedBlock,
{
    /// Verify provided block header/body, typically as part of the block import, without executing
    /// the block.
    ///
    /// Expects (and doesn't check) that `parent_header` correspond to `header`'s parent root,
    /// `header` corresponds to `body` and is internally consistent, see:
    /// * [`Block::is_internally_consistent()`]
    /// * [`BlockHeader::is_internally_consistent()`]
    /// * [`BlockBody::is_internally_consistent()`]
    ///
    /// [`Block::is_internally_consistent()`]: ab_core_primitives::block::Block::is_internally_consistent()
    /// [`BlockHeader::is_internally_consistent()`]: ab_core_primitives::block::header::BlockHeader::is_internally_consistent()
    /// [`BlockBody::is_internally_consistent()`]: ab_core_primitives::block::body::BlockBody::is_internally_consistent()
    ///
    /// These invariants are not checked during verification.
    ///
    /// Since verification doesn't execute the block, state root is ignored and needs to be checked
    /// separately after/if block is re-executed.
    fn verify(
        &self,
        parent_header: &GenericHeader<'_, Block>,
        header: &GenericHeader<'_, Block>,
        body: &GenericBody<'_, Block>,
        origin: BlockOrigin,
    ) -> impl Future<Output = Result<(), BlockVerificationError>> + Send;
}
