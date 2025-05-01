//! Solutions-related data structures and functions.

use crate::block::BlockNumber;
use crate::hashes::{Blake3Hash, blake3_hash_with_key};
use crate::pieces::{PieceOffset, Record, RecordChunk, RecordProof, RecordRoot};
use crate::pos::{PosProof, PosSeed};
use crate::pot::{PotOutput, SlotNumber};
use crate::sectors::{SectorId, SectorIndex, SectorSlotChallenge};
use crate::segments::{HistorySize, SegmentIndex, SegmentRoot};
use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use blake3::OUT_LEN;
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
#[repr(transparent)]
pub struct SolutionDistance(u64);

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
    From,
    Into,
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
                / u128::from(u64::from(era_duration))
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
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(transparent)]
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
            // SAFETY: `ChunkProofHexHash` is `#[repr(transparent)]` and guaranteed to have the
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
            // SAFETY: `ChunkProofHexHash` is `#[repr(transparent)]` and guaranteed to have the
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
    /// Invalid audit chunk offset
    #[error("Invalid audit chunk offset")]
    InvalidAuditChunkOffset,
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
    /// Proof of time for which solution is built
    pub proof_of_time: PotOutput,
    /// Solution range
    pub solution_range: SolutionRange,
    /// Parameters for checking piece validity.
    ///
    /// If `None`, piece validity check will be skipped.
    pub piece_check_params: Option<SolutionVerifyPieceCheckParams>,
}

/// Proof-of-time verifier to be used in [`Solution::verify()`]
pub trait SolutionPotVerifier {
    /// Check whether proof created earlier is valid
    fn is_proof_valid(seed: &PosSeed, challenge_index: u32, proof: &PosProof) -> bool;
}

/// Farmer solution for slot challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Solution {
    /// Public key of the farmer that created the solution
    pub public_key_hash: Blake3Hash,
    /// Index of the sector where the solution was found
    pub sector_index: SectorIndex,
    /// Size of the blockchain history at the time of sector creation
    pub history_size: HistorySize,
    /// Pieces offset within sector
    pub piece_offset: PieceOffset,
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
}

impl Solution {
    /// Fake solution for the genesis block
    pub fn genesis_solution() -> Self {
        Self {
            public_key_hash: Blake3Hash::default(),
            sector_index: SectorIndex::ZERO,
            history_size: HistorySize::from(SegmentIndex::ZERO),
            piece_offset: PieceOffset::default(),
            record_root: RecordRoot::default(),
            record_proof: RecordProof::default(),
            chunk: RecordChunk::default(),
            chunk_proof: ChunkProof::default(),
            proof_of_space: PosProof::default(),
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
            proof_of_time,
            solution_range,
            piece_check_params,
        } = params;

        let sector_id = SectorId::new(&self.public_key_hash, self.sector_index, self.history_size);

        let global_challenge = proof_of_time.derive_global_challenge(slot);
        let sector_slot_challenge = sector_id.derive_sector_slot_challenge(&global_challenge);
        let s_bucket_audit_index = sector_slot_challenge.s_bucket_audit_index();

        // Check that proof of space is valid
        if !PotVerifier::is_proof_valid(
            &sector_id.derive_evaluation_seed(self.piece_offset),
            s_bucket_audit_index.into(),
            &self.proof_of_space,
        ) {
            return Err(SolutionVerifyError::InvalidProofOfSpace);
        };

        let masked_chunk =
            (Simd::from(*self.chunk) ^ Simd::from(*self.proof_of_space.hash())).to_array();

        let solution_distance =
            SolutionDistance::calculate(&global_challenge, &masked_chunk, &sector_slot_challenge);

        if !solution_distance.is_within(*solution_range) {
            return Err(SolutionVerifyError::OutsideSolutionRange {
                solution_range: *solution_range,
                solution_distance,
            });
        }

        // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that allows
        //  the code to compile. Constant 16 is hardcoded here and in `if` branch below for compilation
        //  to succeed
        const _: () = {
            assert!(Record::NUM_S_BUCKETS == 65536);
        };
        // Check that chunk belongs to the record
        if !BalancedHashedMerkleTree::<65536>::verify(
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
