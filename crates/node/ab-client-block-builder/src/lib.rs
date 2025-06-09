#![feature(generic_arg_infer)]

pub mod beacon_chain;

use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::header::{BlockHeaderConsensusInfo, OwnedBlockHeaderSeal};
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, SlotDuration, SlotNumber};
use ab_core_primitives::segments::HistorySize;

// TODO: Probably move it elsewhere
/// Consensus constants
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ConsensusConstants {
    /// Depth `K` after which a block enters the recorded history
    pub confirmation_depth_k: BlockNumber,
    /// Number of slots between slot arrival and when corresponding block can be produced
    pub block_authoring_delay: SlotNumber,
    /// Era duration in blocks
    pub era_duration: BlockNumber,
    /// Slot probability
    pub slot_probability: (u64, u64),
    /// The slot duration in milliseconds
    pub slot_duration: SlotDuration,
    /// Number of latest archived segments that are considered "recent history"
    pub recent_segments: HistorySize,
    /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector
    pub recent_history_fraction: (HistorySize, HistorySize),
    /// Minimum lifetime of a plotted sector, measured in archived segment
    pub min_sector_lifetime: HistorySize,
}

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
