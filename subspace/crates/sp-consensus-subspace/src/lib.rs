//! Primitives for Subspace consensus.

#![forbid(unsafe_code, missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod digests;
pub mod inherents;

use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{
    PotCheckpoints, PotParametersChange, PotSeed, SlotDuration, SlotNumber,
};
use ab_core_primitives::segments::{HistorySize, SegmentHeader, SegmentIndex, SegmentRoot};
use ab_core_primitives::solutions::SolutionRange;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::num::NonZeroU32;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{ConsensusEngineId, Justification};

/// The `ConsensusEngineId` of Subspace.
const SUBSPACE_ENGINE_ID: ConsensusEngineId = *b"SUB_";

/// Subspace justification
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub enum SubspaceJustification {
    /// Proof of time checkpoints that were not seen before
    #[codec(index = 0)]
    PotCheckpoints {
        /// Proof of time seed, the input for computing checkpoints
        seed: PotSeed,
        /// Proof of time checkpoints from after future proof of parent block to current block's
        /// future proof (inclusive)
        checkpoints: Vec<PotCheckpoints>,
    },
}

impl From<SubspaceJustification> for Justification {
    #[inline]
    fn from(justification: SubspaceJustification) -> Self {
        (SUBSPACE_ENGINE_ID, justification.encode())
    }
}

impl SubspaceJustification {
    /// Try to decode Subspace justification from generic justification.
    ///
    /// `None` means this is not a Subspace justification.
    pub fn try_from_justification(
        (consensus_engine_id, encoded_justification): &Justification,
    ) -> Option<Result<Self, parity_scale_codec::Error>> {
        (*consensus_engine_id == SUBSPACE_ENGINE_ID)
            .then(|| Self::decode(&mut encoded_justification.as_slice()))
    }

    /// Returns `true` if justification must be archived, implies that it is canonical
    pub fn must_be_archived(&self) -> bool {
        match self {
            SubspaceJustification::PotCheckpoints { .. } => true,
        }
    }
}

/// An consensus log item for Subspace.
#[derive(Debug, Decode, Encode, Clone, PartialEq, Eq)]
enum ConsensusLog {
    /// Number of iterations for proof of time per slot, corresponds to slot that directly follows
    /// parent block's slot and can change before slot for which block is produced.
    #[codec(index = 0)]
    PotSlotIterations(NonZeroU32),
    /// Solution range for this block/era.
    #[codec(index = 1)]
    SolutionRange(SolutionRange),
    /// Change of parameters to apply to PoT chain.
    #[codec(index = 2)]
    PotParametersChange(PotParametersChange),
    /// Solution range for next block/era.
    #[codec(index = 3)]
    NextSolutionRange(SolutionRange),
    /// Segment roots.
    #[codec(index = 4)]
    SegmentRoot((SegmentIndex, SegmentRoot)),
    /// Enable Solution range adjustment and Override Solution Range.
    #[codec(index = 5)]
    EnableSolutionRangeAdjustmentAndOverride(Option<SolutionRange>),
    /// Root plot public key was updated.
    #[codec(index = 6)]
    RootPlotPublicKeyHashUpdate(Option<Blake3Hash>),
}

/// Subspace solution ranges used for challenges.
#[derive(Decode, Encode, MaxEncodedLen, PartialEq, Eq, Clone, Copy, Debug, TypeInfo)]
pub struct SolutionRanges {
    /// Solution range in current block/era.
    pub current: SolutionRange,
    /// Solution range that will be used in the next block/era.
    pub next: Option<SolutionRange>,
}

impl Default for SolutionRanges {
    #[inline]
    fn default() -> Self {
        Self {
            current: SolutionRange::MAX,
            next: None,
        }
    }
}

/// Subspace blockchain constants.
#[derive(Debug, Encode, Decode, PartialEq, Eq, Clone, Copy, TypeInfo)]
pub enum ChainConstants {
    /// V0 of the chain constants.
    #[codec(index = 0)]
    V0 {
        /// Depth `K` after which a block enters the recorded history.
        confirmation_depth_k: BlockNumber,
        /// Number of slots between slot arrival and when corresponding block can be produced.
        block_authoring_delay: SlotNumber,
        /// Era duration in blocks.
        era_duration: BlockNumber,
        /// Slot probability.
        slot_probability: (u64, u64),
        /// The slot duration in milliseconds.
        slot_duration: SlotDuration,
        /// Number of latest archived segments that are considered "recent history".
        recent_segments: HistorySize,
        /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector.
        recent_history_fraction: (HistorySize, HistorySize),
        /// Minimum lifetime of a plotted sector, measured in archived segment.
        min_sector_lifetime: HistorySize,
    },
}

impl ChainConstants {
    /// Depth `K` after which a block enters the recorded history.
    pub fn confirmation_depth_k(&self) -> BlockNumber {
        let Self::V0 {
            confirmation_depth_k,
            ..
        } = self;
        *confirmation_depth_k
    }

    /// Era duration in blocks.
    pub fn era_duration(&self) -> BlockNumber {
        let Self::V0 { era_duration, .. } = self;
        *era_duration
    }

    /// Number of slots between slot arrival and when corresponding block can be produced.
    pub fn block_authoring_delay(&self) -> SlotNumber {
        let Self::V0 {
            block_authoring_delay,
            ..
        } = self;
        *block_authoring_delay
    }

    /// Slot probability.
    pub fn slot_probability(&self) -> (u64, u64) {
        let Self::V0 {
            slot_probability, ..
        } = self;
        *slot_probability
    }

    /// The slot duration in milliseconds.
    pub fn slot_duration(&self) -> SlotDuration {
        let Self::V0 { slot_duration, .. } = self;
        *slot_duration
    }

    /// Number of latest archived segments that are considered "recent history".
    pub fn recent_segments(&self) -> HistorySize {
        let Self::V0 {
            recent_segments, ..
        } = self;
        *recent_segments
    }

    /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector.
    pub fn recent_history_fraction(&self) -> (HistorySize, HistorySize) {
        let Self::V0 {
            recent_history_fraction,
            ..
        } = self;
        *recent_history_fraction
    }

    /// Minimum lifetime of a plotted sector, measured in archived segment.
    pub fn min_sector_lifetime(&self) -> HistorySize {
        let Self::V0 {
            min_sector_lifetime,
            ..
        } = self;
        *min_sector_lifetime
    }
}

/// Proof of time parameters
#[derive(Debug, Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct PotParameters {
    /// Number of iterations for proof of time per slot, corresponds to slot that directly
    /// follows parent block's slot and can change before slot for which block is produced
    pub slot_iterations: NonZeroU32,
    /// Optional next scheduled change of parameters
    pub next_change: Option<PotParametersChange>,
}

sp_api::decl_runtime_apis! {
    /// API necessary for block authorship with Subspace.
    pub trait SubspaceApi {
        /// Proof of time parameters
        fn pot_parameters() -> PotParameters;

        /// Solution ranges.
        fn solution_ranges() -> SolutionRanges;

        /// Size of the blockchain history
        fn history_size() -> HistorySize;

        /// How many pieces one sector is supposed to contain (max)
        fn max_pieces_in_sector() -> u16;

        /// Get the segment root of records for specified segment index
        fn segment_root(segment_index: SegmentIndex) -> Option<SegmentRoot>;

        /// Returns `Vec<SegmentHeader>` if a given extrinsic has them.
        fn extract_segment_headers(ext: &Block::Extrinsic) -> Option<Vec<SegmentHeader >>;

        /// Checks if the extrinsic is an inherent.
        fn is_inherent(ext: &Block::Extrinsic) -> bool;

        /// Returns root plot public key hash in case block authoring is restricted.
        fn root_plot_public_key_hash() -> Option<Blake3Hash>;

        /// Whether solution range adjustment is enabled.
        fn should_adjust_solution_range() -> bool;

        /// Get Subspace blockchain constants
        fn chain_constants() -> ChainConstants;
    }
}
