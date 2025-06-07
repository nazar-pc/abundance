//! Block building implementation

use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::pot::{SlotDuration, SlotNumber};
use ab_core_primitives::segments::HistorySize;

pub mod slot_worker;

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
