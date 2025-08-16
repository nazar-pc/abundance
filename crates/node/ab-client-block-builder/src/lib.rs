//! Block building implementation

#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![feature(async_fn_traits, unboxed_closures)]

pub mod beacon_chain;

use ab_client_api::BlockDetails;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::header::{BlockHeaderConsensusInfo, OwnedBlockHeaderSeal};
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::PotCheckpoints;

/// Error for [`BlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderError {
    /// Invalid parent MMR
    #[error("Invalid parent MMR")]
    InvalidParentMmr,
    /// Custom builder error
    #[error("Custom builder error: {error}")]
    Custom {
        // Custom block builder error
        #[from]
        error: anyhow::Error,
    },
    /// Failed to seal the block
    #[error("Failed to seal the block")]
    FailedToSeal,
    /// Received invalid seal
    #[error(
        "Received invalid seal for pre-seal hash {pre_seal_hash} and public key hash \
        {public_key_hash}"
    )]
    InvalidSeal {
        /// Public key hash
        public_key_hash: Blake3Hash,
        /// Pre-seal hash
        pre_seal_hash: Blake3Hash,
    },
    /// Can't extend MMR, too many blocks; this is an implementation bug and must never happen
    #[error(
        "Can't extend MMR, too many blocks; this is an implementation bug and must never happen"
    )]
    CantExtendMmr,
}

/// Result of block building
#[derive(Debug, Clone)]
pub struct BlockBuilderResult<Block> {
    /// Block itself
    pub block: Block,
    /// Additional details about a block
    pub block_details: BlockDetails,
}

/// Block builder interface
pub trait BlockBuilder<Block>: Send
where
    Block: GenericOwnedBlock,
{
    /// Build a new block using provided parameters
    fn build<SealBlock>(
        &mut self,
        parent_block_root: &BlockRoot,
        parent_header: &<Block::Header as GenericOwnedBlockHeader>::Header<'_>,
        parent_block_details: &BlockDetails,
        consensus_info: &BlockHeaderConsensusInfo,
        checkpoints: &[PotCheckpoints],
        seal_block: SealBlock,
    ) -> impl Future<Output = Result<BlockBuilderResult<Block>, BlockBuilderError>> + Send
    where
        SealBlock: AsyncFnOnce<(Blake3Hash,), Output = Option<OwnedBlockHeaderSeal>, CallOnceFuture: Send>
            + Send;
}
