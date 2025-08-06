//! Segments-related data structures.

#[cfg(feature = "alloc")]
mod archival_history_segment;

use crate::block::BlockNumber;
use crate::hashes::Blake3Hash;
use crate::pieces::{PieceIndex, Record};
#[cfg(feature = "alloc")]
pub use crate::segments::archival_history_segment::ArchivedHistorySegment;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::iter::Step;
use core::num::{NonZeroU32, NonZeroU64};
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, Deref, DerefMut, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub,
    SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Super segment root contained within beacon chain block
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct SuperSegmentRoot([u8; SuperSegmentRoot::SIZE]);

impl fmt::Debug for SuperSegmentRoot {
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
struct SuperSegmentRootBinary(#[serde(with = "BigArray")] [u8; SuperSegmentRoot::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SuperSegmentRootHex(#[serde(with = "hex")] [u8; SuperSegmentRoot::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for SuperSegmentRoot {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            SuperSegmentRootHex(self.0).serialize(serializer)
        } else {
            SuperSegmentRootBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for SuperSegmentRoot {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            SuperSegmentRootHex::deserialize(deserializer)?.0
        } else {
            SuperSegmentRootBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for SuperSegmentRoot {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl AsRef<[u8]> for SuperSegmentRoot {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for SuperSegmentRoot {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl SuperSegmentRoot {
    /// Size in bytes
    pub const SIZE: usize = 32;
}

/// Segment index type.
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
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SegmentIndex(u64);

impl Step for SegmentIndex {
    #[inline]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u64::steps_between(&start.0, &end.0)
    }

    #[inline]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u64::forward_checked(start.0, count).map(Self)
    }

    #[inline]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u64::backward_checked(start.0, count).map(Self)
    }
}

impl SegmentIndex {
    /// Segment index 0.
    pub const ZERO: SegmentIndex = SegmentIndex(0);
    /// Segment index 1.
    pub const ONE: SegmentIndex = SegmentIndex(1);

    /// Create a new instance
    #[inline]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Get the first piece index in this segment.
    #[inline]
    pub const fn first_piece_index(&self) -> PieceIndex {
        PieceIndex::new(self.0 * RecordedHistorySegment::NUM_PIECES as u64)
    }

    /// Get the last piece index in this segment.
    #[inline]
    pub const fn last_piece_index(&self) -> PieceIndex {
        PieceIndex::new((self.0 + 1) * RecordedHistorySegment::NUM_PIECES as u64 - 1)
    }

    /// List of piece indexes that belong to this segment.
    #[inline]
    pub fn segment_piece_indexes(&self) -> [PieceIndex; RecordedHistorySegment::NUM_PIECES] {
        let mut piece_indices = [PieceIndex::ZERO; RecordedHistorySegment::NUM_PIECES];
        (self.first_piece_index()..=self.last_piece_index())
            .zip(&mut piece_indices)
            .for_each(|(input, output)| {
                *output = input;
            });

        piece_indices
    }

    /// Checked integer subtraction. Computes `self - rhs`, returning `None` if underflow occurred.
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Saturating integer subtraction. Computes `self - rhs`, returning zero if underflow
    /// occurred.
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Segment root contained within segment header.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[repr(C)]
pub struct SegmentRoot([u8; SegmentRoot::SIZE]);

impl fmt::Debug for SegmentRoot {
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
struct SegmentRootBinary(#[serde(with = "BigArray")] [u8; SegmentRoot::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SegmentRootHex(#[serde(with = "hex")] [u8; SegmentRoot::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for SegmentRoot {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            SegmentRootHex(self.0).serialize(serializer)
        } else {
            SegmentRootBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for SegmentRoot {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            SegmentRootHex::deserialize(deserializer)?.0
        } else {
            SegmentRootBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for SegmentRoot {
    #[inline(always)]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl AsRef<[u8]> for SegmentRoot {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for SegmentRoot {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl SegmentRoot {
    /// Size in bytes
    pub const SIZE: usize = 32;

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `SegmentRoot` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `SegmentRoot` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Size of blockchain history in segments.
#[derive(
    Debug,
    Display,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    From,
    Into,
    Deref,
    DerefMut,
    TrivialType,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
// Storing `SegmentIndex` to make all invariants valid
pub struct HistorySize(SegmentIndex);

impl HistorySize {
    /// History size of one
    pub const ONE: Self = Self(SegmentIndex::ZERO);

    /// Create a new instance
    #[inline(always)]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(SegmentIndex::new(value.get() - 1))
    }

    /// Get internal representation
    pub const fn as_segment_index(&self) -> SegmentIndex {
        self.0
    }

    /// Get internal representation
    pub const fn as_non_zero_u64(&self) -> NonZeroU64 {
        NonZeroU64::new(self.0.as_u64().saturating_add(1)).expect("Not zero; qed")
    }

    /// Size of blockchain history in pieces.
    #[inline(always)]
    pub const fn in_pieces(&self) -> NonZeroU64 {
        NonZeroU64::new(
            self.0
                .as_u64()
                .saturating_add(1)
                .saturating_mul(RecordedHistorySegment::NUM_PIECES as u64),
        )
        .expect("Not zero; qed")
    }

    /// Segment index that corresponds to this history size.
    #[inline(always)]
    pub fn segment_index(&self) -> SegmentIndex {
        self.0
    }

    /// History size at which expiration check for sector happens.
    ///
    /// Returns `None` on overflow.
    #[inline(always)]
    pub fn sector_expiration_check(&self, min_sector_lifetime: Self) -> Option<Self> {
        self.as_non_zero_u64()
            .checked_add(min_sector_lifetime.as_non_zero_u64().get())
            .map(Self::new)
    }
}

/// Progress of an archived block.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct ArchivedBlockProgress {
    /// Number of partially archived bytes of a block, `0` for full block
    bytes: u32,
}

impl Default for ArchivedBlockProgress {
    /// We assume a block can always fit into the segment initially, but it is definitely possible
    /// to be transitioned into the partial state after some overflow checking.
    #[inline(always)]
    fn default() -> Self {
        Self::new_complete()
    }
}

impl ArchivedBlockProgress {
    /// Block is archived fully
    #[inline(always)]
    pub const fn new_complete() -> Self {
        Self { bytes: 0 }
    }

    /// Block is partially archived with provided number of bytes
    #[inline(always)]
    pub const fn new_partial(new_partial: NonZeroU32) -> Self {
        Self {
            bytes: new_partial.get(),
        }
    }

    /// Return the number of partially archived bytes if the progress is not complete
    #[inline(always)]
    pub const fn partial(&self) -> Option<NonZeroU32> {
        NonZeroU32::new(self.bytes)
    }
}

/// Last archived block
#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct LastArchivedBlock {
    /// Block number
    pub number: Unaligned<BlockNumber>,
    /// Progress of an archived block.
    pub archived_progress: ArchivedBlockProgress,
}

impl LastArchivedBlock {
    /// Returns the number of partially archived bytes for a block.
    #[inline(always)]
    pub fn partial_archived(&self) -> Option<NonZeroU32> {
        self.archived_progress.partial()
    }

    /// Sets the number of partially archived bytes if block progress was archived partially
    #[inline(always)]
    pub fn set_partial_archived(&mut self, new_partial: NonZeroU32) {
        self.archived_progress = ArchivedBlockProgress::new_partial(new_partial);
    }

    /// Indicate last archived block was archived fully
    #[inline(always)]
    pub fn set_complete(&mut self) {
        self.archived_progress = ArchivedBlockProgress::new_complete();
    }

    /// Get block number (unwrap `Unaligned`)
    pub const fn number(&self) -> BlockNumber {
        self.number.as_inner()
    }
}

/// Segment header for a specific segment.
///
/// Each segment will have corresponding [`SegmentHeader`] included as the first item in the next
/// segment. Each `SegmentHeader` includes hash of the previous one and all together form a chain of
/// segment headers that is used for quick and efficient verification that some `Piece`
/// corresponds to the actual archival history of the blockchain.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct SegmentHeader {
    /// Segment index
    pub segment_index: Unaligned<SegmentIndex>,
    /// Root of roots of all records in a segment.
    pub segment_root: SegmentRoot,
    /// Hash of the segment header of the previous segment
    pub prev_segment_header_hash: Blake3Hash,
    /// Last archived block
    pub last_archived_block: LastArchivedBlock,
}

impl SegmentHeader {
    /// Hash of the whole segment header
    #[inline(always)]
    pub fn hash(&self) -> Blake3Hash {
        blake3::hash(self.as_bytes()).into()
    }

    /// Get segment index (unwrap `Unaligned`)
    #[inline(always)]
    pub const fn segment_index(&self) -> SegmentIndex {
        self.segment_index.as_inner()
    }
}

/// Recorded history segment before archiving is applied.
///
/// NOTE: This is a stack-allocated data structure and can cause stack overflow!
#[derive(Copy, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(C)]
pub struct RecordedHistorySegment([Record; Self::NUM_RAW_RECORDS]);

impl fmt::Debug for RecordedHistorySegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordedHistorySegment")
            .finish_non_exhaustive()
    }
}

impl Default for RecordedHistorySegment {
    #[inline]
    fn default() -> Self {
        Self([Record::default(); Self::NUM_RAW_RECORDS])
    }
}

impl AsRef<[u8]> for RecordedHistorySegment {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        Record::slice_to_repr(&self.0).as_flattened().as_flattened()
    }
}

impl AsMut<[u8]> for RecordedHistorySegment {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        Record::slice_mut_to_repr(&mut self.0)
            .as_flattened_mut()
            .as_flattened_mut()
    }
}

impl RecordedHistorySegment {
    /// Number of raw records in one segment of recorded history.
    pub const NUM_RAW_RECORDS: usize = 128;
    /// Erasure coding rate for records during archiving process.
    pub const ERASURE_CODING_RATE: (usize, usize) = (1, 2);
    /// Number of pieces in one segment of archived history (taking erasure coding rate into
    /// account)
    pub const NUM_PIECES: usize =
        Self::NUM_RAW_RECORDS * Self::ERASURE_CODING_RATE.1 / Self::ERASURE_CODING_RATE.0;
    /// Size of recorded history segment in bytes.
    ///
    /// It includes half of the records (just source records) that will later be erasure coded and
    /// together with corresponding roots and proofs will result in
    /// [`Self::NUM_PIECES`] `Piece`s of archival history.
    pub const SIZE: usize = Record::SIZE * Self::NUM_RAW_RECORDS;

    /// Create boxed value without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        // SAFETY: Data structure filled with zeroes is a valid invariant
        unsafe { Box::<Self>::new_zeroed().assume_init() }
    }
}
