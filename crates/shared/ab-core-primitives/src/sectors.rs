//! Sectors-related data structures.

#[cfg(test)]
mod tests;

use crate::hashes::Blake3Hash;
use crate::nano_u256::NanoU256;
use crate::pieces::{PieceIndex, PieceOffset, Record};
use crate::pos::PosSeed;
use crate::segments::{HistorySize, SegmentRoot};
use crate::solutions::ShardCommitmentHash;
use ab_blake3::{single_block_hash, single_block_keyed_hash};
use ab_io_type::trivial_type::TrivialType;
use core::hash::Hash;
use core::iter::Step;
use core::num::{NonZeroU64, TryFromIntError};
use core::simd::Simd;
use derive_more::{
    Add, AddAssign, Deref, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Sector index in consensus
#[derive(
    Debug,
    Display,
    Default,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    From,
    Into,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    Div,
    DivAssign,
    TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SectorIndex(u16);

impl Step for SectorIndex {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u16::steps_between(&start.0, &end.0)
    }

    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u16::forward_checked(start.0, count).map(Self)
    }

    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u16::backward_checked(start.0, count).map(Self)
    }
}

impl From<SectorIndex> for u32 {
    #[inline(always)]
    fn from(original: SectorIndex) -> Self {
        u32::from(original.0)
    }
}

impl From<SectorIndex> for u64 {
    #[inline(always)]
    fn from(original: SectorIndex) -> Self {
        u64::from(original.0)
    }
}

impl From<SectorIndex> for usize {
    #[inline(always)]
    fn from(original: SectorIndex) -> Self {
        usize::from(original.0)
    }
}

impl SectorIndex {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u16>();
    /// Sector index 0
    pub const ZERO: Self = Self(0);
    /// Max sector index
    pub const MAX: Self = Self(u16::MAX);

    /// Create a new instance
    #[inline(always)]
    pub const fn new(n: u16) -> Self {
        Self(n)
    }

    /// Create sector index from bytes.
    #[inline(always)]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u16::from_le_bytes(bytes))
    }

    /// Convert sector index to bytes.
    #[inline(always)]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }
}

/// Challenge used for a particular sector for particular slot
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deref)]
pub struct SectorSlotChallenge(Blake3Hash);

impl SectorSlotChallenge {
    /// Index of s-bucket within sector to be audited
    #[inline]
    pub fn s_bucket_audit_index(&self) -> SBucket {
        // As long as number of s-buckets is 2^16, we can pick first two bytes instead of actually
        // calculating `U256::from_le_bytes(self.0) % Record::NUM_S_BUCKETS)`
        const {
            assert!(Record::NUM_S_BUCKETS == 1 << u16::BITS as usize);
        }
        SBucket::from(u16::from_le_bytes([self.0[0], self.0[1]]))
    }
}

/// Data structure representing sector ID in farmer's plot
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SectorId(Blake3Hash);

impl AsRef<[u8]> for SectorId {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl SectorId {
    /// Size in bytes
    const SIZE: usize = Blake3Hash::SIZE;

    /// Create a new sector ID by deriving it from public key and sector index
    #[inline]
    pub fn new(
        public_key_hash: &Blake3Hash,
        shard_commitments_root: &ShardCommitmentHash,
        sector_index: SectorIndex,
        history_size: HistorySize,
    ) -> Self {
        let mut bytes_to_hash =
            [0; SectorIndex::SIZE + HistorySize::SIZE as usize + ShardCommitmentHash::SIZE];
        bytes_to_hash[..SectorIndex::SIZE].copy_from_slice(&sector_index.to_bytes());
        bytes_to_hash[SectorIndex::SIZE..][..HistorySize::SIZE as usize]
            .copy_from_slice(&history_size.as_non_zero_u64().get().to_le_bytes());
        bytes_to_hash[SectorIndex::SIZE + HistorySize::SIZE as usize..]
            .copy_from_slice(shard_commitments_root.as_bytes());
        // TODO: Is keyed hash really needed here?
        Self(Blake3Hash::new(
            single_block_keyed_hash(public_key_hash, &bytes_to_hash)
                .expect("Less than a single block worth of bytes; qed"),
        ))
    }

    /// Derive piece index that should be stored in sector at `piece_offset` for specified size of
    /// blockchain history
    pub fn derive_piece_index(
        &self,
        piece_offset: PieceOffset,
        history_size: HistorySize,
        max_pieces_in_sector: u16,
        recent_segments: HistorySize,
        recent_history_fraction: (HistorySize, HistorySize),
    ) -> PieceIndex {
        let recent_segments_in_pieces = recent_segments.in_pieces().get();
        // Recent history must be at most `recent_history_fraction` of all history to use separate
        // policy for recent pieces
        let min_history_size_in_pieces = recent_segments_in_pieces
            * recent_history_fraction.1.in_pieces().get()
            / recent_history_fraction.0.in_pieces().get();
        let input_hash = {
            let piece_offset_bytes = piece_offset.to_bytes();
            let mut key = [0; 32];
            key[..piece_offset_bytes.len()].copy_from_slice(&piece_offset_bytes);
            // TODO: Is keyed hash really needed here?
            NanoU256::from_le_bytes(
                single_block_keyed_hash(&key, self.as_ref())
                    .expect("Less than a single block worth of bytes; qed"),
            )
        };
        let history_size_in_pieces = history_size.in_pieces().get();
        let num_interleaved_pieces = 1.max(
            u64::from(max_pieces_in_sector) * recent_history_fraction.0.in_pieces().get()
                / recent_history_fraction.1.in_pieces().get()
                * 2,
        );

        let piece_index = if history_size_in_pieces > min_history_size_in_pieces
            && u64::from(piece_offset) < num_interleaved_pieces
            && u16::from(piece_offset) % 2 == 1
        {
            // For odd piece offsets at the beginning of the sector pick pieces at random from
            // recent history only
            (input_hash % recent_segments_in_pieces)
                + (history_size_in_pieces - recent_segments_in_pieces)
        } else {
            input_hash % history_size_in_pieces
        };

        PieceIndex::from(piece_index)
    }

    /// Derive sector slot challenge for this sector from provided global challenge
    pub fn derive_sector_slot_challenge(
        &self,
        global_challenge: &Blake3Hash,
    ) -> SectorSlotChallenge {
        let sector_slot_challenge = Simd::from(*self.0) ^ Simd::from(**global_challenge);
        SectorSlotChallenge(sector_slot_challenge.to_array().into())
    }

    /// Derive evaluation seed
    pub fn derive_evaluation_seed(&self, piece_offset: PieceOffset) -> PosSeed {
        let mut bytes_to_hash = [0; Self::SIZE + PieceOffset::SIZE];
        bytes_to_hash[..Self::SIZE].copy_from_slice(self.as_ref());
        bytes_to_hash[Self::SIZE..].copy_from_slice(&piece_offset.to_bytes());
        let evaluation_seed = single_block_hash(&bytes_to_hash)
            .expect("Less than a single block worth of bytes; qed");

        PosSeed::from(evaluation_seed)
    }

    /// Derive history size when sector created at `history_size` expires.
    ///
    /// Returns `None` on overflow.
    pub fn derive_expiration_history_size(
        &self,
        history_size: HistorySize,
        sector_expiration_check_segment_root: &SegmentRoot,
        min_sector_lifetime: HistorySize,
    ) -> Option<HistorySize> {
        let sector_expiration_check_history_size = history_size
            .sector_expiration_check(min_sector_lifetime)?
            .as_non_zero_u64();

        let input_hash = NanoU256::from_le_bytes(
            single_block_hash([*self.0, **sector_expiration_check_segment_root].as_flattened())
                .expect("Less than a single block worth of bytes; qed"),
        );

        let last_possible_expiration = min_sector_lifetime
            .as_non_zero_u64()
            .checked_add(history_size.as_non_zero_u64().get().checked_mul(4u64)?)?;
        let expires_in = input_hash
            % last_possible_expiration
                .get()
                .checked_sub(sector_expiration_check_history_size.get())?;

        let expiration_history_size = sector_expiration_check_history_size.get() + expires_in;
        let expiration_history_size = NonZeroU64::try_from(expiration_history_size).expect(
            "History size is not zero, so result is not zero even if expires immediately; qed",
        );
        Some(HistorySize::new(expiration_history_size))
    }
}

/// S-bucket used in consensus
#[derive(
    Debug,
    Display,
    Default,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    From,
    Into,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    Div,
    DivAssign,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SBucket(u16);

impl Step for SBucket {
    #[inline]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u16::steps_between(&start.0, &end.0)
    }

    #[inline]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u16::forward_checked(start.0, count).map(Self)
    }

    #[inline]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u16::backward_checked(start.0, count).map(Self)
    }
}

impl TryFrom<usize> for SBucket {
    type Error = TryFromIntError;

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Self(u16::try_from(value)?))
    }
}

impl From<SBucket> for u32 {
    #[inline]
    fn from(original: SBucket) -> Self {
        u32::from(original.0)
    }
}

impl From<SBucket> for usize {
    #[inline]
    fn from(original: SBucket) -> Self {
        usize::from(original.0)
    }
}

impl SBucket {
    /// S-bucket 0.
    pub const ZERO: SBucket = SBucket(0);
    /// Max s-bucket index
    pub const MAX: SBucket = SBucket((Record::NUM_S_BUCKETS - 1) as u16);
}
