use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::header::{BlockHeaderConsensusInfo, OwnedBlockHeaderSeal};
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::PotCheckpoints;

/// Error for [`BlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderError {
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
}

/// Block builder interface
pub trait BlockBuilder<Block>
where
    Block: GenericOwnedBlock,
{
    /// Build a new block using provided parameters
    fn build<SealBlock, SealBlockFut>(
        &mut self,
        parent_block_root: &BlockRoot,
        parent_header: &<Block::Header as GenericOwnedBlockHeader>::Header<'_>,
        consensus_info: &BlockHeaderConsensusInfo,
        checkpoints: &[PotCheckpoints],
        seal_block: SealBlock,
    ) -> impl Future<Output = Result<Block, BlockBuilderError>> + Send
    where
        SealBlock: FnOnce(Blake3Hash) -> SealBlockFut + Send,
        SealBlockFut: Future<Output = Option<OwnedBlockHeaderSeal>> + Send;
}
