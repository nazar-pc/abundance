//! Solutions-related data structures and functions.

use crate::hashes::{blake3_hash_with_key, Blake3Hash};
use crate::pieces::{PieceOffset, Record, RecordChunk, RecordProof, RecordRoot};
use crate::pos::PosProof;
use crate::sectors::{SectorIndex, SectorSlotChallenge};
use crate::segments::{HistorySize, SegmentIndex};
use crate::{BlockNumber, PublicKey, SlotNumber};
use blake3::OUT_LEN;
use core::array::TryFromSliceError;
use core::fmt;
use derive_more::{
    Add, AddAssign, AsMut, AsRef, Deref, DerefMut, Display, From, Into, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde::{Deserializer, Serializer};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;
use static_assertions::const_assert;

/// Solution distance
#[derive(Debug, Display, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct SolutionDistance(u64);

impl From<u64> for SolutionDistance {
    #[inline]
    fn from(original: u64) -> Self {
        Self(original)
    }
}

impl From<SolutionDistance> for u64 {
    #[inline]
    fn from(original: SolutionDistance) -> Self {
        original.0
    }
}

impl SolutionDistance {
    /// Maximum value
    pub const MAX: Self = Self(u64::MAX / 2);

    // TODO: Remove once `From` is stable
    /// Create new instance
    #[inline(always)]
    pub const fn from_u64(n: u64) -> Self {
        Self(n)
    }

    /// Calculate solution distance for given parameters.
    ///
    /// Typically used as a primitive to check whether solution distance is within solution range
    /// (see [`Self::is_within()`]).
    pub fn calculate(
        global_challenge: &Blake3Hash,
        chunk: &[u8; 32],
        sector_slot_challenge: &SectorSlotChallenge,
    ) -> Self {
        let audit_chunk = blake3_hash_with_key(sector_slot_challenge, chunk);
        let audit_chunk_as_solution_range: SolutionRange = SolutionRange::from_bytes(
            *audit_chunk
                .array_chunks::<{ SolutionRange::SIZE }>()
                .next()
                .expect("Solution range is smaller in size than global challenge; qed"),
        );
        let global_challenge_as_solution_range: SolutionRange = SolutionRange::from_bytes(
            *global_challenge
                .array_chunks::<{ SolutionRange::SIZE }>()
                .next()
                .expect("Solution range is smaller in size than global challenge; qed"),
        );

        global_challenge_as_solution_range.bidirectional_distance(audit_chunk_as_solution_range)
    }

    /// Check if solution distance is within the provided solution range
    pub const fn is_within(self, solution_range: SolutionRange) -> bool {
        self.0 <= solution_range.to_u64() / 2
    }
}

/// Solution range
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
    Add,
    AddAssign,
    Sub,
    SubAssign,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct SolutionRange(u64);

impl From<u64> for SolutionRange {
    #[inline]
    fn from(original: u64) -> Self {
        Self(original)
    }
}

impl From<SolutionRange> for u64 {
    #[inline]
    fn from(original: SolutionRange) -> Self {
        original.0
    }
}

impl SolutionRange {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u64>();
    /// Minimum value
    pub const MIN: Self = Self(u64::MIN);
    /// Maximum value
    pub const MAX: Self = Self(u64::MAX);

    // TODO: Remove once `From` is stable
    /// Create new instance
    #[inline(always)]
    pub const fn from_u64(n: u64) -> Self {
        Self(n)
    }

    // TODO: Remove once `From` is stable
    /// Get internal representation
    #[inline(always)]
    pub const fn to_u64(self) -> u64 {
        self.0
    }

    /// Create a new instance from bytes
    #[inline(always)]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Computes the following:
    /// ```text
    /// MAX * slot_probability / chunks * s_buckets / pieces
    /// ```
    #[inline]
    pub const fn from_pieces(pieces: u64, slot_probability: (u64, u64)) -> Self {
        let solution_range = u64::MAX
            // Account for slot probability
            / slot_probability.1 * slot_probability.0
            // Now take the probability of hitting occupied s-bucket in a piece into account
            / Record::NUM_CHUNKS as u64
            * Record::NUM_S_BUCKETS as u64;

        // Take the number of pieces into account
        Self(solution_range / pieces)
    }

    /// Computes the following:
    /// ```text
    /// MAX * slot_probability / chunks * s_buckets / solution_range
    /// ```
    #[inline]
    pub const fn to_pieces(self, slot_probability: (u64, u64)) -> u64 {
        let pieces = u64::MAX
            // Account for slot probability
            / slot_probability.1 * slot_probability.0
            // Now take the probability of hitting occupied s-bucket in sector into account
            / Record::NUM_CHUNKS as u64
            * Record::NUM_S_BUCKETS as u64;

        // Take solution range into account
        pieces / self.0
    }

    /// Bidirectional distance between two solution ranges
    #[inline]
    pub const fn bidirectional_distance(self, other: Self) -> SolutionDistance {
        let a = self.0;
        let b = other.0;
        let diff = a.wrapping_sub(b);
        let diff2 = b.wrapping_sub(a);
        // Find smaller diff between 2 directions
        SolutionDistance::from_u64(if diff < diff2 { diff } else { diff2 })
    }

    /// Derives next solution range based on the total era slots and slot probability
    pub fn derive_next(
        self,
        start_slot: SlotNumber,
        current_slot: SlotNumber,
        slot_probability: (u64, u64),
        era_duration: BlockNumber,
    ) -> Self {
        // calculate total slots within this era
        let era_slot_count = current_slot - start_slot;

        // The idea here is to keep block production at the same pace while space pledged on the
        // network changes. For this, we adjust the previous solution range according to actual and
        // expected number of blocks per era.
        //
        // Below is code analogous to the following, but without using floats:
        // ```rust
        // let actual_slots_per_block = era_slot_count as f64 / era_duration as f64;
        // let expected_slots_per_block =
        //     slot_probability.1 as f64 / slot_probability.0 as f64;
        // let adjustment_factor =
        //     (actual_slots_per_block / expected_slots_per_block).clamp(0.25, 4.0);
        //
        // next_solution_range =
        //     (solution_ranges.current as f64 * adjustment_factor).round() as u64;
        // ```
        let current_solution_range = self.0;
        let next_solution_range = u64::try_from(
            u128::from(current_solution_range)
                .saturating_mul(u128::from(era_slot_count))
                .saturating_mul(u128::from(slot_probability.0))
                / u128::from(era_duration)
                / u128::from(slot_probability.1),
        );

        Self(next_solution_range.unwrap_or(u64::MAX).clamp(
            current_solution_range / 4,
            current_solution_range.saturating_mul(4),
        ))
    }
}

// Quick test to ensure functions above are the inverse of each other
const_assert!(SolutionRange::from_pieces(1, (1, 6)).to_pieces((1, 6)) == 1);
const_assert!(SolutionRange::from_pieces(3, (1, 6)).to_pieces((1, 6)) == 3);
const_assert!(SolutionRange::from_pieces(5, (1, 6)).to_pieces((1, 6)) == 5);

/// A Ristretto Schnorr signature as bytes produced by `schnorrkel` crate.
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Deref, From, Into)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
pub struct RewardSignature([u8; RewardSignature::SIZE]);

impl fmt::Debug for RewardSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RewardSignatureBinary(#[serde(with = "BigArray")] [u8; RewardSignature::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RewardSignatureHex(#[serde(with = "hex")] [u8; RewardSignature::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for RewardSignature {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RewardSignatureHex(self.0).serialize(serializer)
        } else {
            RewardSignatureBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RewardSignature {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RewardSignatureHex::deserialize(deserializer)?.0
        } else {
            RewardSignatureBinary::deserialize(deserializer)?.0
        }))
    }
}

impl AsRef<[u8]> for RewardSignature {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl RewardSignature {
    /// Reward signature size in bytes
    pub const SIZE: usize = 64;
}

/// Proof for chunk contained within a record.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(transparent)]
pub struct ChunkProof([u8; ChunkProof::SIZE]);

impl fmt::Debug for ChunkProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ChunkProofBinary(#[serde(with = "BigArray")] [u8; ChunkProof::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ChunkProofHex(#[serde(with = "hex")] [u8; ChunkProof::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for ChunkProof {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            ChunkProofHex(self.0).serialize(serializer)
        } else {
            ChunkProofBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ChunkProof {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            ChunkProofHex::deserialize(deserializer)?.0
        } else {
            ChunkProofBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for ChunkProof {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl TryFrom<&[u8]> for ChunkProof {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <[u8; Self::SIZE]>::try_from(slice).map(Self)
    }
}

impl AsRef<[u8]> for ChunkProof {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for ChunkProof {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl ChunkProof {
    /// Size of chunk proof in bytes.
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = Record::NUM_S_BUCKETS.ilog2() as usize;
}

/// Farmer solution for slot challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Solution {
    /// Public key of the farmer that created the solution
    pub public_key: PublicKey,
    /// Index of the sector where solution was found
    pub sector_index: SectorIndex,
    /// Size of the blockchain history at time of sector creation
    pub history_size: HistorySize,
    /// Pieces offset within sector
    pub piece_offset: PieceOffset,
    /// Record root that can use used to verify that piece was included in blockchain history
    pub record_root: RecordRoot,
    /// Proof for above record root
    pub record_proof: RecordProof,
    /// Chunk at above offset
    pub chunk: RecordChunk,
    /// Proof for above chunk
    pub chunk_proof: ChunkProof,
    /// Proof of space for piece offset
    pub proof_of_space: PosProof,
}

impl Solution {
    /// Dummy solution for the genesis block
    pub fn genesis_solution(public_key: PublicKey) -> Self {
        Self {
            public_key,
            sector_index: 0,
            history_size: HistorySize::from(SegmentIndex::ZERO),
            piece_offset: PieceOffset::default(),
            record_root: RecordRoot::default(),
            record_proof: RecordProof::default(),
            chunk: RecordChunk::default(),
            chunk_proof: ChunkProof::default(),
            proof_of_space: PosProof::default(),
        }
    }
}
