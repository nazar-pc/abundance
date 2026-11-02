#![feature(option_into_flat_iter)]

pub mod consensus_parameters;
pub mod state;
#[cfg(test)]
mod test_utils;

use ab_core_primitives::block::{BlockNumber, BlockTimestamp};
use ab_core_primitives::pot::{SlotDuration, SlotNumber};
use ab_core_primitives::segments::HistorySize;
use futures::channel::mpsc;

/// Proof-of-time consensus constants
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct PotConsensusConstants {
    /// Interval, in blocks, between blockchain entropy injection into the proof of time chain
    pub entropy_injection_interval: BlockNumber,
    /// Interval, in entropy injection intervals, where to take entropy for injection from
    pub entropy_injection_lookback_depth: u8,
    /// Delay after block, in slots, when entropy injection takes effect
    pub entropy_injection_delay: SlotNumber,
}

/// Consensus constants
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ConsensusConstants {
    /// Depth at which a block is confirmed and enters the recorded history.
    ///
    /// When block `n` is built, the block at `n - block_confirmation_depth` is confirmed, so the
    /// largest possible reorg is `block_confirmation_depth - 1` blocks. Importing the next best
    /// block (block number `best_number + 1`) confirms the block at
    /// `best_number + 1 - block_confirmation_depth`, so fork blocks must be above it, and blocks
    /// up to it are the same on every chain that can still be extended. This is why they can
    /// be referenced by block number alone.
    ///
    /// With `block_confirmation_depth = 5` and `shard_confirmation_depth = 2`:
    ///
    /// ```text
    /// block number
    /// best_number + 1  next best block
    /// best_number      best block
    /// best_number - 3  deepest fork blocks
    /// best_number - 4  confirmed once the next best block is imported  -+ read by the super
    /// best_number - 5  confirmed                                        | segment derivation of
    /// best_number - 6  confirmed                                       -+ the next best block
    /// ```
    ///
    /// The block lifecycle in the database is shown on `ClientDatabaseOptions` in
    /// `ab-client-database`.
    ///
    /// This is from the perspective of the individual shard, with additional confirmation on the
    /// beacon chain required for intermediate and leaf shards (see `shard_confirmation_depth`).
    pub block_confirmation_depth: BlockNumber,
    /// Depth on the beacon chain in addition to `block_confirmation_depth` after intermediate or
    /// leaf shard information on the beacon chain is confirmed and corresponding segment roots
    /// become a part of the super segment.
    ///
    /// Segment roots included in the beacon chain block at
    /// `n - block_confirmation_depth - shard_confirmation_depth` become a part of the super
    /// segment of block `n` unless later beacon chain blocks up to `n - block_confirmation_depth`
    /// show that their shard reorged them (see the diagram on `block_confirmation_depth`).
    ///
    /// This is separate from the block confirmation on the corresponding shard itself, which
    /// happens much sooner (see `block_confirmation_depth`), but can't be immediately trusted by
    /// the beacon chain.
    pub shard_confirmation_depth: BlockNumber,
    /// Number of slots between slot arrival and when the corresponding block can be produced
    pub block_authoring_delay: SlotNumber,
    /// Proof-of-time consensus constants
    pub pot: PotConsensusConstants,
    // TODO: Non-zero block number would be nice
    /// Period of time in blocks after which the solution range is adjusted
    pub retarget_interval: BlockNumber,
    /// Slot probability
    pub slot_probability: (u64, u64),
    /// The slot duration in milliseconds
    pub slot_duration: SlotDuration,
    /// Number of latest archived segments that are considered "recent history"
    pub recent_segments: HistorySize,
    /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector
    pub recent_history_fraction: (HistorySize, HistorySize),
    /// Minimum lifetime of a plotted sector, measured in archived segments
    pub min_sector_lifetime: HistorySize,
    /// Max block timestamp drift allowed
    pub max_block_timestamp_drift: BlockTimestamp,
    // TODO: Non-zero block number would be nice
    /// Number of beacon chain blocks between shard rotations.
    ///
    /// Every this number of beacon chain blocks PoT entropy is taken to calculate the next shard
    /// assignment.
    pub shard_rotation_interval: BlockNumber,
    /// Delay in beacon chain blocks for the next shard rotation.
    ///
    /// Delay after shard assignment is revealed before it actually takes effect (essentially the
    /// amount of time for a node to sync the corresponding shard).
    pub shard_rotation_delay: BlockNumber,
}

/// Notification with information about the block that is about to be imported and acknowledgement
/// sender that can be used to pause block production if necessary
#[derive(Debug, Clone)]
pub struct BlockImportingNotification {
    /// Block number
    pub block_number: BlockNumber,
    /// Sender for pausing the block import for archiving purposes is not fast enough to process
    /// the consensus block
    pub acknowledgement_sender: mpsc::Sender<()>,
}
