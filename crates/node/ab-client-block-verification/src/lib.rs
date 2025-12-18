#![feature(array_windows)]

pub mod beacon_chain;

use ab_client_api::BlockOrigin;
use ab_client_consensus_common::consensus_parameters::DeriveConsensusParametersChainInfo;
use ab_core_primitives::block::body::owned::GenericOwnedBlockBody;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::SegmentRoot;

type GenericHeader<'a, Block> =
    <<Block as GenericOwnedBlock>::Header as GenericOwnedBlockHeader>::Header<'a>;
type GenericBody<'a, Block> =
    <<Block as GenericOwnedBlock>::Body as GenericOwnedBlockBody>::Body<'a>;

/// Error for [`BlockVerification`]
#[derive(Debug, thiserror::Error)]
pub enum BlockVerificationError {
    /// Block is below the archiving point
    #[error("Block is below archiving point")]
    BelowArchivingPoint,
    /// Invalid header prefix
    #[error("Invalid header prefix")]
    InvalidHeaderPrefix,
    /// Timestamp too far in the future
    #[error("Timestamp too far in the future")]
    TimestampTooFarInTheFuture,
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
    /// Verify the provided block header/body, typically as part of the block import, without
    /// executing the block.
    ///
    /// This method can be called concurrently even for interdependent blocks.
    ///
    /// Expects (and doesn't check) that `parent_header` corresponds to `header`'s parent root,
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
    /// Since verification doesn't execute the block, the state root is ignored and needs to be
    /// checked separately after/if the block is re-executed.
    fn verify_concurrent<BCI>(
        &self,
        parent_header: &GenericHeader<'_, Block>,
        parent_block_mmr_root: &Blake3Hash,
        header: &GenericHeader<'_, Block>,
        body: &GenericBody<'_, Block>,
        origin: &BlockOrigin,
        beacon_chain_info: &BCI,
    ) -> impl Future<Output = Result<(), BlockVerificationError>> + Send
    where
        BCI: DeriveConsensusParametersChainInfo;

    /// Complementary to [`Self::verify_concurrent()`] that expects the parent block to be already
    /// successfully imported.
    fn verify_sequential(
        &self,
        parent_header: &GenericHeader<'_, Block>,
        parent_block_mmr_root: &Blake3Hash,
        header: &GenericHeader<'_, Block>,
        body: &GenericBody<'_, Block>,
        origin: &BlockOrigin,
    ) -> impl Future<Output = Result<(), BlockVerificationError>> + Send;
}
