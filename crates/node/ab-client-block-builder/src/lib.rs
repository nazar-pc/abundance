use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::{BlockHeaderConsensusInfo, BlockHeaderSeal};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotSeed};

/// Error for [`BlockBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum BlockBuilderError {
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
pub trait BlockBuilder<Block> {
    /// Build a new block using provided parameters
    fn build<SealBlock, SealBlockFut>(
        &mut self,
        parent_block_root: &BlockRoot,
        consensus_info: &BlockHeaderConsensusInfo,
        seed: &PotSeed,
        checkpoints: &[PotCheckpoints],
        seal_block: SealBlock,
    ) -> Result<Block, BlockBuilderError>
    where
        SealBlock: FnOnce(Blake3Hash) -> SealBlockFut,
        SealBlockFut: Future<Output = Option<BlockHeaderSeal>>;
}
