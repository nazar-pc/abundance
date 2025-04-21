//! Solutions-related data structures and functions.

use crate::pieces::{PieceOffset, Record, RecordChunk, RecordProof, RecordRoot};
use crate::pos::PosProof;
use crate::sectors::SectorIndex;
use crate::segments::{HistorySize, SegmentIndex};
use crate::PublicKey;
use blake3::OUT_LEN;
use core::array::TryFromSliceError;
use core::fmt;
use derive_more::{AsMut, AsRef, Deref, DerefMut, From, Into};
use num_traits::WrappingSub;
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

// TODO: Add related methods to `SolutionRange`.
/// Type of solution range.
pub type SolutionRange = u64;

/// Computes the following:
/// ```text
/// MAX * slot_probability / chunks * s_buckets / pieces
/// ```
pub const fn pieces_to_solution_range(pieces: u64, slot_probability: (u64, u64)) -> SolutionRange {
    let solution_range = SolutionRange::MAX
        // Account for slot probability
        / slot_probability.1 * slot_probability.0
        // Now take probability of hitting occupied s-bucket in a piece into account
        / Record::NUM_CHUNKS as u64
        * Record::NUM_S_BUCKETS as u64;

    // Take number of pieces into account
    solution_range / pieces
}

/// Computes the following:
/// ```text
/// MAX * slot_probability / chunks * s_buckets / solution_range
/// ```
pub const fn solution_range_to_pieces(
    solution_range: SolutionRange,
    slot_probability: (u64, u64),
) -> u64 {
    let pieces = SolutionRange::MAX
        // Account for slot probability
        / slot_probability.1 * slot_probability.0
        // Now take probability of hitting occupied s-bucket in sector into account
        / Record::NUM_CHUNKS as u64
        * Record::NUM_S_BUCKETS as u64;

    // Take solution range into account
    pieces / solution_range
}

// Quick test to ensure functions above are the inverse of each other
const_assert!(solution_range_to_pieces(pieces_to_solution_range(1, (1, 6)), (1, 6)) == 1);
const_assert!(solution_range_to_pieces(pieces_to_solution_range(3, (1, 6)), (1, 6)) == 3);
const_assert!(solution_range_to_pieces(pieces_to_solution_range(5, (1, 6)), (1, 6)) == 5);

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

/// Bidirectional distance metric implemented on top of subtraction
#[inline(always)]
pub fn bidirectional_distance<T: WrappingSub + Ord>(a: &T, b: &T) -> T {
    let diff = a.wrapping_sub(b);
    let diff2 = b.wrapping_sub(a);
    // Find smaller diff between 2 directions.
    diff.min(diff2)
}
