//! Solutions-related data structures and functions.

use crate::block::BlockNumber;
use crate::ed25519::Ed25519PublicKey;
use crate::hashes::Blake3Hash;
use crate::pieces::{PieceOffset, Record, RecordChunk, RecordProof, RecordRoot};
use crate::pos::{PosProof, PosSeed};
use crate::pot::{PotOutput, SlotNumber};
use crate::sectors::{SBucket, SectorId, SectorIndex, SectorSlotChallenge};
use crate::segments::{HistorySize, SegmentIndex, SegmentRoot};
use crate::shard::{NumShards, RealShardKind, ShardIndex, ShardKind};
use ab_blake3::single_block_keyed_hash;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::balanced::BalancedMerkleTree;
use blake3::{Hash, OUT_LEN};
use core::simd::Simd;
use core::{fmt, mem};
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

/// Solution distance
#[derive(
    Debug, Display, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, From, Into,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SolutionDistance(u64);

impl SolutionDistance {
    /// Maximum value
    pub const MAX: Self = Self(u64::MAX / 2);

    // TODO: Remove once `From` is stable
    /// Create a new instance
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
        // TODO: Is keyed hash really needed here?
        let audit_chunk = single_block_keyed_hash(sector_slot_challenge, chunk)
            .expect("Less than a single block worth of bytes; qed");
        let audit_chunk_as_solution_range: SolutionRange = SolutionRange::from_bytes([
            audit_chunk[0],
            audit_chunk[1],
            audit_chunk[2],
            audit_chunk[3],
            audit_chunk[4],
            audit_chunk[5],
            audit_chunk[6],
            audit_chunk[7],
        ]);
        let global_challenge_as_solution_range: SolutionRange =
            SolutionRange::from_bytes(global_challenge.as_chunks().0[0]);

        global_challenge_as_solution_range.bidirectional_distance(audit_chunk_as_solution_range)
    }

    /// Check if solution distance is within the provided solution range
    pub const fn is_within(self, solution_range: SolutionRange) -> bool {
        self.0 <= solution_range.as_u64() / 2
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
    From,
    Into,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SolutionRange(u64);

impl SolutionRange {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u64>();
    /// Minimum value
    pub const MIN: Self = Self(u64::MIN);
    /// Maximum value
    pub const MAX: Self = Self(u64::MAX);

    // TODO: Remove once `From` is stable
    /// Create a new instance
    #[inline(always)]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    // TODO: Remove once `From` is stable
    /// Get internal representation
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create a new instance from bytes
    #[inline(always)]
    pub fn to_bytes(self) -> [u8; 8] {
        self.0.to_le_bytes()
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

    /// Expands the global solution range to a solution range that corresponds to a leaf shard.
    ///
    /// Global solution range is updated based on the beacon chain information, while a farmer also
    /// creates intermediate shard and leaf shard solutions with a wider solution range.
    #[inline]
    pub const fn to_leaf_shard(self, num_shards: NumShards) -> Self {
        Self(
            self.0
                .saturating_mul(u64::from(num_shards.leaf_shards().get())),
        )
    }

    /// Expands the global solution range to a solution range that corresponds to an intermediate
    /// shard
    #[inline]
    pub const fn to_intermediate_shard(self, num_shards: NumShards) -> Self {
        Self(
            self.0
                .saturating_mul(u64::from(num_shards.intermediate_shards().get())),
        )
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

    /// Derives next solution range
    #[inline]
    pub fn derive_next(
        self,
        slots_in_last_interval: SlotNumber,
        slot_probability: (u64, u64),
        retarget_interval: BlockNumber,
    ) -> Self {
        // The idea here is to keep block production at the same pace while space pledged on the
        // network changes. For this, we adjust the previous solution range according to actual and
        // expected number of blocks per retarget interval.
        //
        // Below is code analogous to the following, but without using floats:
        // ```rust
        // let actual_slots_per_block = slots_in_last_interval as f64 / retarget_interval as f64;
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
                .saturating_mul(u128::from(slots_in_last_interval))
                .saturating_mul(u128::from(slot_probability.0))
                / u128::from(u64::from(retarget_interval))
                / u128::from(slot_probability.1),
        );

        Self(next_solution_range.unwrap_or(u64::MAX).clamp(
            current_solution_range / 4,
            current_solution_range.saturating_mul(4),
        ))
    }
}

// Quick test to ensure the functions above are the inverse of each other
const _: () = {
    assert!(SolutionRange::from_pieces(1, (1, 6)).to_pieces((1, 6)) == 1);
    assert!(SolutionRange::from_pieces(3, (1, 6)).to_pieces((1, 6)) == 3);
    assert!(SolutionRange::from_pieces(5, (1, 6)).to_pieces((1, 6)) == 5);
};

/// Proof for chunk contained within a record.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct ChunkProof([[u8; OUT_LEN]; ChunkProof::NUM_HASHES]);

impl fmt::Debug for ChunkProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for hash in self.0 {
            for byte in hash {
                write!(f, "{byte:02x}")?;
            }
            write!(f, ", ")?;
        }
        write!(f, "]")?;
        Ok(())
    }
}

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ChunkProofBinary(#[serde(with = "BigArray")] [[u8; OUT_LEN]; ChunkProof::NUM_HASHES]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ChunkProofHexHash(#[serde(with = "hex")] [u8; OUT_LEN]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ChunkProofHex([ChunkProofHexHash; ChunkProof::NUM_HASHES]);

#[cfg(feature = "serde")]
impl Serialize for ChunkProof {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // SAFETY: `ChunkProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            ChunkProofHex(unsafe {
                mem::transmute::<
                    [[u8; OUT_LEN]; ChunkProof::NUM_HASHES],
                    [ChunkProofHexHash; ChunkProof::NUM_HASHES],
                >(self.0)
            })
            .serialize(serializer)
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
            // SAFETY: `ChunkProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            unsafe {
                mem::transmute::<
                    [ChunkProofHexHash; ChunkProof::NUM_HASHES],
                    [[u8; OUT_LEN]; ChunkProof::NUM_HASHES],
                >(ChunkProofHex::deserialize(deserializer)?.0)
            }
        } else {
            ChunkProofBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for ChunkProof {
    #[inline]
    fn default() -> Self {
        Self([[0; OUT_LEN]; ChunkProof::NUM_HASHES])
    }
}

impl AsRef<[u8]> for ChunkProof {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_flattened()
    }
}

impl AsMut<[u8]> for ChunkProof {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_flattened_mut()
    }
}

impl ChunkProof {
    /// Size of chunk proof in bytes.
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = Record::NUM_S_BUCKETS.ilog2() as usize;
}

/// Solution verification errors
#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum SolutionVerifyError {
    /// Invalid piece offset
    #[error("Piece verification failed")]
    InvalidPieceOffset {
        /// Index of the piece that failed verification
        piece_offset: u16,
        /// How many pieces one sector is supposed to contain (max)
        max_pieces_in_sector: u16,
    },
    /// History size is in the future
    #[error("History size {solution} is in the future, current is {current}")]
    FutureHistorySize {
        /// Current history size
        current: HistorySize,
        /// History size solution was created for
        solution: HistorySize,
    },
    /// Sector expired
    #[error("Sector expired")]
    SectorExpired {
        /// Expiration history size
        expiration_history_size: HistorySize,
        /// Current history size
        current_history_size: HistorySize,
    },
    /// Piece verification failed
    #[error("Piece verification failed")]
    InvalidPiece,
    /// Solution is outside the solution range
    #[error("Solution distance {solution_distance} is outside of solution range {solution_range}")]
    OutsideSolutionRange {
        /// Solution range
        solution_range: SolutionRange,
        /// Solution distance
        solution_distance: SolutionDistance,
    },
    /// Invalid proof of space
    #[error("Invalid proof of space")]
    InvalidProofOfSpace,
    /// Invalid shard commitment
    #[error("Invalid shard commitment")]
    InvalidShardCommitment,
    /// Invalid input shard
    #[error("Invalid input shard {shard_index} ({shard_kind:?})")]
    InvalidInputShard {
        /// Input shard index
        shard_index: ShardIndex,
        /// Input shard kind
        shard_kind: Option<ShardKind>,
    },
    /// Invalid solution shard
    #[error(
        "Invalid solution shard {solution_shard_index} (parent {solution_parent_shard_index:?}), \
        expected shard {expected_shard_index} ({expected_shard_kind:?})"
    )]
    InvalidSolutionShard {
        /// Solution shard index
        solution_shard_index: ShardIndex,
        /// Solution shard index
        solution_parent_shard_index: Option<ShardIndex>,
        /// Expected shard index
        expected_shard_index: ShardIndex,
        /// Expected shard kind
        expected_shard_kind: RealShardKind,
    },
    /// Invalid chunk proof
    #[error("Invalid chunk proof")]
    InvalidChunkProof,
    /// Invalid history size
    #[error("Invalid history size")]
    InvalidHistorySize,
}

/// Parameters for checking piece validity
#[derive(Debug, Clone)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
pub struct SolutionVerifyPieceCheckParams {
    /// How many pieces one sector is supposed to contain (max)
    pub max_pieces_in_sector: u16,
    /// Segment root of the segment to which piece belongs
    pub segment_root: SegmentRoot,
    /// Number of latest archived segments that are considered "recent history"
    pub recent_segments: HistorySize,
    /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector
    pub recent_history_fraction: (HistorySize, HistorySize),
    /// Minimum lifetime of a plotted sector, measured in archived segments
    pub min_sector_lifetime: HistorySize,
    /// Current size of the history
    pub current_history_size: HistorySize,
    /// Segment root at `min_sector_lifetime` from sector creation (if exists)
    pub sector_expiration_check_segment_root: Option<SegmentRoot>,
}

/// Parameters for solution verification
#[derive(Debug, Clone)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
pub struct SolutionVerifyParams {
    /// Shard for which the solution is built
    pub shard_index: ShardIndex,
    /// Proof of time for which solution is built
    pub proof_of_time: PotOutput,
    /// Solution range
    pub solution_range: SolutionRange,
    /// Shard membership entropy
    pub shard_membership_entropy: ShardMembershipEntropy,
    /// The number of shards in the network
    pub num_shards: NumShards,
    /// Parameters for checking piece validity.
    ///
    /// If `None`, piece validity check will be skipped.
    pub piece_check_params: Option<SolutionVerifyPieceCheckParams>,
}

/// Proof-of-time verifier to be used in [`Solution::verify()`]
pub trait SolutionPotVerifier {
    /// Check whether proof created earlier is valid
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool;
}

/// Entropy used for shard membership assignment
#[derive(
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    From,
    Into,
    AsRef,
    AsMut,
    Deref,
    DerefMut,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct ShardMembershipEntropy([u8; ShardMembershipEntropy::SIZE]);

impl fmt::Display for ShardMembershipEntropy {
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
struct ShardMembershipEntropyBinary([u8; ShardMembershipEntropy::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ShardMembershipEntropyHex(#[serde(with = "hex")] [u8; ShardMembershipEntropy::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for ShardMembershipEntropy {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            ShardMembershipEntropyHex(self.0).serialize(serializer)
        } else {
            ShardMembershipEntropyBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ShardMembershipEntropy {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            ShardMembershipEntropyHex::deserialize(deserializer)?.0
        } else {
            ShardMembershipEntropyBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Debug for ShardMembershipEntropy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for ShardMembershipEntropy {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for ShardMembershipEntropy {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl ShardMembershipEntropy {
    /// Size in bytes
    pub const SIZE: usize = PotOutput::SIZE;

    /// Create a new instance
    #[inline(always)]
    pub const fn new(bytes: [u8; Self::SIZE]) -> Self {
        Self(bytes)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &self.0
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `ShardMembershipEntropy` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `ShardMembershipEntropy` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

/// Reduced hash used for shard assignment
#[derive(
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    From,
    Into,
    AsRef,
    AsMut,
    Deref,
    DerefMut,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct ShardCommitmentHash([u8; ShardCommitmentHash::SIZE]);

impl fmt::Display for ShardCommitmentHash {
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
struct ShardCommitmentHashBinary([u8; ShardCommitmentHash::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ShardCommitmentHashHex(#[serde(with = "hex")] [u8; ShardCommitmentHash::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for ShardCommitmentHash {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            ShardCommitmentHashHex(self.0).serialize(serializer)
        } else {
            ShardCommitmentHashBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ShardCommitmentHash {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            ShardCommitmentHashHex::deserialize(deserializer)?.0
        } else {
            ShardCommitmentHashBinary::deserialize(deserializer)?.0
        }))
    }
}

impl fmt::Debug for ShardCommitmentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for ShardCommitmentHash {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for ShardCommitmentHash {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<Hash> for ShardCommitmentHash {
    #[inline(always)]
    fn from(value: Hash) -> Self {
        let bytes = value.as_bytes();
        Self(*bytes)
        // Self([
        //     bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        // ])
    }
}

impl ShardCommitmentHash {
    // TODO: Reduce to 8 bytes once Merkle Tree implementation exists that produces such hashes
    /// Size in bytes
    pub const SIZE: usize = 32;

    /// Create a new instance
    #[inline(always)]
    pub const fn new(hash: [u8; Self::SIZE]) -> Self {
        Self(hash)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &self.0
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `ShardCommitmentHash` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from array of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn array_from_repr<const N: usize>(value: [[u8; Self::SIZE]; N]) -> [Self; N] {
        // TODO: Should have been transmute, but https://github.com/rust-lang/rust/issues/61956
        // SAFETY: `ShardCommitmentHash` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute_copy(&value) }
    }

    /// Convenient conversion to a slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `ShardCommitmentHash` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to an array of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_array<const N: usize>(value: [Self; N]) -> [[u8; Self::SIZE]; N] {
        // TODO: Should have been transmute, but https://github.com/rust-lang/rust/issues/61956
        // SAFETY: `ShardCommitmentHash` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute_copy(&value) }
    }
}

/// Information about shard commitments in the solution
#[derive(Clone, Copy, Debug, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct SolutionShardCommitment {
    /// Root of the Merkle Tree of shard commitments
    pub root: ShardCommitmentHash,
    /// Proof for the shard commitment used the solution
    pub proof: [ShardCommitmentHash; SolutionShardCommitment::NUM_LEAVES.ilog2() as usize],
    /// Shard commitment leaf used for the solution
    pub leaf: ShardCommitmentHash,
}

impl SolutionShardCommitment {
    /// Number of leaves in a Merkle Tree of shard commitments
    pub const NUM_LEAVES: usize = 2_u32.pow(20) as usize;
}

/// Farmer solution for slot challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct Solution {
    /// Public key of the farmer that created the solution
    pub public_key_hash: Blake3Hash,
    /// Farmer's shard commitment
    pub shard_commitment: SolutionShardCommitment,
    /// Record root that can use used to verify that piece was included in blockchain history
    pub record_root: RecordRoot,
    /// Proof for above record root
    pub record_proof: RecordProof,
    /// Chunk at the above offset
    pub chunk: RecordChunk,
    /// Proof for the above chunk
    pub chunk_proof: ChunkProof,
    /// Proof of space for piece offset
    pub proof_of_space: PosProof,
    /// Size of the blockchain history at the time of sector creation
    pub history_size: HistorySize,
    /// Index of the sector where the solution was found
    pub sector_index: SectorIndex,
    /// Pieces offset within sector
    pub piece_offset: PieceOffset,
    /// Padding for data structure alignment
    pub padding: [u8; 4],
}

impl Solution {
    /// Fake solution for the genesis block
    pub fn genesis_solution() -> Self {
        Self {
            public_key_hash: Ed25519PublicKey::default().hash(),
            shard_commitment: SolutionShardCommitment {
                root: Default::default(),
                proof: [Default::default(); _],
                leaf: Default::default(),
            },
            record_root: RecordRoot::default(),
            record_proof: RecordProof::default(),
            chunk: RecordChunk::default(),
            chunk_proof: ChunkProof::default(),
            proof_of_space: PosProof::default(),
            history_size: HistorySize::from(SegmentIndex::ZERO),
            sector_index: SectorIndex::ZERO,
            piece_offset: PieceOffset::default(),
            padding: [0; _],
        }
    }

    /// Check solution validity
    pub fn verify<PotVerifier>(
        &self,
        slot: SlotNumber,
        params: &SolutionVerifyParams,
    ) -> Result<(), SolutionVerifyError>
    where
        PotVerifier: SolutionPotVerifier,
    {
        let SolutionVerifyParams {
            shard_index,
            proof_of_time,
            solution_range,
            shard_membership_entropy,
            num_shards,
            piece_check_params,
        } = params;

        let shard_kind = shard_index
            .shard_kind()
            .and_then(|shard_kind| shard_kind.to_real())
            .ok_or(SolutionVerifyError::InvalidInputShard {
                shard_index: *shard_index,
                shard_kind: shard_index.shard_kind(),
            })?;

        let (solution_shard_index, shard_commitment_index) = num_shards
            .derive_shard_index_and_shard_commitment_index(
                &self.public_key_hash,
                &self.shard_commitment.root,
                shard_membership_entropy,
                self.history_size,
            );

        // Adjust solution range according to shard kind
        let solution_range = match shard_kind {
            RealShardKind::BeaconChain => *solution_range,
            RealShardKind::IntermediateShard => {
                if solution_shard_index.parent_shard() != Some(*shard_index) {
                    return Err(SolutionVerifyError::InvalidSolutionShard {
                        solution_shard_index,
                        solution_parent_shard_index: solution_shard_index.parent_shard(),
                        expected_shard_index: *shard_index,
                        expected_shard_kind: RealShardKind::IntermediateShard,
                    });
                }

                solution_range.to_intermediate_shard(*num_shards)
            }
            RealShardKind::LeafShard => {
                if solution_shard_index != *shard_index {
                    return Err(SolutionVerifyError::InvalidSolutionShard {
                        solution_shard_index,
                        solution_parent_shard_index: solution_shard_index.parent_shard(),
                        expected_shard_index: *shard_index,
                        expected_shard_kind: RealShardKind::LeafShard,
                    });
                }

                solution_range.to_leaf_shard(*num_shards)
            }
        };

        // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that
        //  allows the code to compile. Constant 65536 is hardcoded here and below for compilation
        //  to succeed.
        const {
            assert!(SolutionShardCommitment::NUM_LEAVES == 1048576);
        }
        if !BalancedMerkleTree::<1048576>::verify(
            &self.shard_commitment.root,
            &ShardCommitmentHash::repr_from_array(self.shard_commitment.proof),
            shard_commitment_index as usize,
            *self.shard_commitment.leaf,
        ) {
            return Err(SolutionVerifyError::InvalidShardCommitment);
        }

        let sector_id = SectorId::new(
            &self.public_key_hash,
            &self.shard_commitment.root,
            self.sector_index,
            self.history_size,
        );

        let global_challenge = proof_of_time.derive_global_challenge(slot);
        let sector_slot_challenge = sector_id.derive_sector_slot_challenge(&global_challenge);
        let s_bucket_audit_index = sector_slot_challenge.s_bucket_audit_index();

        // Check that proof of space is valid
        if !PotVerifier::is_proof_valid(
            &sector_id.derive_evaluation_seed(self.piece_offset),
            s_bucket_audit_index,
            &self.proof_of_space,
        ) {
            return Err(SolutionVerifyError::InvalidProofOfSpace);
        };

        let masked_chunk =
            (Simd::from(*self.chunk) ^ Simd::from(*self.proof_of_space.hash())).to_array();

        let solution_distance =
            SolutionDistance::calculate(&global_challenge, &masked_chunk, &sector_slot_challenge);

        if !solution_distance.is_within(solution_range) {
            return Err(SolutionVerifyError::OutsideSolutionRange {
                solution_range,
                solution_distance,
            });
        }

        // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that
        //  allows the code to compile. Constant 65536 is hardcoded here and below for compilation
        //  to succeed.
        const {
            assert!(Record::NUM_S_BUCKETS == 65536);
        }
        // Check that chunk belongs to the record
        if !BalancedMerkleTree::<65536>::verify(
            &self.record_root,
            &self.chunk_proof,
            usize::from(s_bucket_audit_index),
            *self.chunk,
        ) {
            return Err(SolutionVerifyError::InvalidChunkProof);
        }

        if let Some(SolutionVerifyPieceCheckParams {
            max_pieces_in_sector,
            segment_root,
            recent_segments,
            recent_history_fraction,
            min_sector_lifetime,
            current_history_size,
            sector_expiration_check_segment_root,
        }) = piece_check_params
        {
            if &self.history_size > current_history_size {
                return Err(SolutionVerifyError::FutureHistorySize {
                    current: *current_history_size,
                    solution: self.history_size,
                });
            }

            if u16::from(self.piece_offset) >= *max_pieces_in_sector {
                return Err(SolutionVerifyError::InvalidPieceOffset {
                    piece_offset: u16::from(self.piece_offset),
                    max_pieces_in_sector: *max_pieces_in_sector,
                });
            }

            if let Some(sector_expiration_check_segment_root) = sector_expiration_check_segment_root
            {
                let expiration_history_size = match sector_id.derive_expiration_history_size(
                    self.history_size,
                    sector_expiration_check_segment_root,
                    *min_sector_lifetime,
                ) {
                    Some(expiration_history_size) => expiration_history_size,
                    None => {
                        return Err(SolutionVerifyError::InvalidHistorySize);
                    }
                };

                if expiration_history_size <= *current_history_size {
                    return Err(SolutionVerifyError::SectorExpired {
                        expiration_history_size,
                        current_history_size: *current_history_size,
                    });
                }
            }

            let position = sector_id
                .derive_piece_index(
                    self.piece_offset,
                    self.history_size,
                    *max_pieces_in_sector,
                    *recent_segments,
                    *recent_history_fraction,
                )
                .position();

            // Check that piece is part of the blockchain history
            if !self
                .record_root
                .is_valid(segment_root, &self.record_proof, position)
            {
                return Err(SolutionVerifyError::InvalidPiece);
            }
        }

        Ok(())
    }
}
