//! Pieces-related data structures.

#[cfg(feature = "alloc")]
mod cow_bytes;
#[cfg(feature = "alloc")]
mod flat_pieces;
#[cfg(feature = "alloc")]
mod piece;

#[cfg(feature = "alloc")]
pub use crate::pieces::flat_pieces::FlatPieces;
#[cfg(feature = "alloc")]
pub use crate::pieces::piece::Piece;
#[cfg(feature = "alloc")]
pub use crate::segments::ArchivedHistorySegment;
use crate::segments::{RecordedHistorySegment, SegmentIndex};
use crate::ScalarBytes;
#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use ::serde::{Deserializer, Serializer};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use blake3::OUT_LEN;
use core::array::TryFromSliceError;
use core::hash::Hash;
use core::iter::Step;
#[cfg(feature = "alloc")]
use core::slice;
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, AsMut, AsRef, Deref, DerefMut, Display, Div, DivAssign, From, Into, Mul,
    MulAssign, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Piece index in consensus
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
    Mul,
    MulAssign,
    Div,
    DivAssign,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct PieceIndex(u64);

impl Step for PieceIndex {
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

impl From<u64> for PieceIndex {
    #[inline]
    fn from(original: u64) -> Self {
        Self(original)
    }
}

impl From<PieceIndex> for u64 {
    #[inline]
    fn from(original: PieceIndex) -> Self {
        original.0
    }
}

impl PieceIndex {
    /// Size in bytes.
    pub const SIZE: usize = mem::size_of::<u64>();
    /// Piece index 0.
    pub const ZERO: PieceIndex = PieceIndex(0);
    /// Piece index 1.
    pub const ONE: PieceIndex = PieceIndex(1);

    /// Create new instance
    #[inline]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Create piece index from bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert piece index to bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }

    /// Segment index piece index corresponds to
    #[inline]
    pub const fn segment_index(&self) -> SegmentIndex {
        SegmentIndex::new(self.0 / RecordedHistorySegment::NUM_PIECES as u64)
    }

    /// Position of a piece in a segment
    #[inline]
    pub const fn position(&self) -> u32 {
        // Position is statically guaranteed to fit into u32
        (self.0 % RecordedHistorySegment::NUM_PIECES as u64) as u32
    }

    /// Position of a source piece in the source pieces for a segment.
    /// Panics if the piece is not a source piece.
    #[inline]
    pub const fn source_position(&self) -> u32 {
        assert!(self.is_source());

        let source_start = self.position() / RecordedHistorySegment::ERASURE_CODING_RATE.1 as u32
            * RecordedHistorySegment::ERASURE_CODING_RATE.0 as u32;
        let source_offset = self.position() % RecordedHistorySegment::ERASURE_CODING_RATE.1 as u32;

        source_start + source_offset
    }

    /// Returns the piece index for a source position and segment index.
    /// Overflows to the next segment if the position is greater than the last source position.
    #[inline]
    pub const fn from_source_position(
        source_position: u32,
        segment_index: SegmentIndex,
    ) -> PieceIndex {
        let source_position = source_position as u64;
        let start = source_position / RecordedHistorySegment::ERASURE_CODING_RATE.0 as u64
            * RecordedHistorySegment::ERASURE_CODING_RATE.1 as u64;
        let offset = source_position % RecordedHistorySegment::ERASURE_CODING_RATE.0 as u64;

        PieceIndex(segment_index.first_piece_index().0 + start + offset)
    }

    /// Is this piece index a source piece?
    #[inline]
    pub const fn is_source(&self) -> bool {
        // Source pieces are interleaved with parity pieces, source first
        self.0 % (RecordedHistorySegment::ERASURE_CODING_RATE.1 as u64)
            < (RecordedHistorySegment::ERASURE_CODING_RATE.0 as u64)
    }

    /// Returns the next source piece index.
    /// Panics if the piece is not a source piece.
    #[inline]
    pub const fn next_source_index(&self) -> PieceIndex {
        PieceIndex::from_source_position(self.source_position() + 1, self.segment_index())
    }
}

/// Piece offset in sector
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
    Mul,
    MulAssign,
    Div,
    DivAssign,
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, MaxEncodedLen, TypeInfo)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct PieceOffset(u16);

impl Step for PieceOffset {
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

impl From<u16> for PieceOffset {
    #[inline]
    fn from(original: u16) -> Self {
        Self(original)
    }
}

impl From<PieceOffset> for u16 {
    #[inline]
    fn from(original: PieceOffset) -> Self {
        original.0
    }
}

impl From<PieceOffset> for u32 {
    #[inline]
    fn from(original: PieceOffset) -> Self {
        Self::from(original.0)
    }
}

impl From<PieceOffset> for u64 {
    #[inline]
    fn from(original: PieceOffset) -> Self {
        Self::from(original.0)
    }
}

impl From<PieceOffset> for usize {
    #[inline]
    fn from(original: PieceOffset) -> Self {
        usize::from(original.0)
    }
}

impl PieceOffset {
    /// Piece index 0.
    pub const ZERO: PieceOffset = PieceOffset(0);
    /// Piece index 1.
    pub const ONE: PieceOffset = PieceOffset(1);

    /// Convert piece offset to bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; mem::size_of::<u16>()] {
        self.0.to_le_bytes()
    }
}

/// Record contained within a piece.
///
/// NOTE: This is a stack-allocated data structure and can cause stack overflow!
#[derive(Copy, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(transparent)]
pub struct Record([[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]);

impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_flattened() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Default for Record {
    #[inline]
    fn default() -> Self {
        Self([Default::default(); Record::NUM_CHUNKS])
    }
}

impl AsRef<[u8]> for Record {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_flattened()
    }
}

impl AsMut<[u8]> for Record {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_flattened_mut()
    }
}

impl From<&Record> for &[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]> for &Record {
    #[inline]
    fn from(value: &[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut Record> for &mut [[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &mut Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]> for &mut Record {
    #[inline]
    fn from(value: &mut [[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&Record> for &[u8; Record::SIZE] {
    #[inline]
    fn from(value: &Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        // as inner array, while array of byte arrays has the same alignment as a single byte
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; Record::SIZE]> for &Record {
    #[inline]
    fn from(value: &[u8; Record::SIZE]) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        // as inner array, while array of byte arrays has the same alignment as a single byte
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut Record> for &mut [u8; Record::SIZE] {
    #[inline]
    fn from(value: &mut Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        // as inner array, while array of byte arrays has the same alignment as a single byte
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; Record::SIZE]> for &mut Record {
    #[inline]
    fn from(value: &mut [u8; Record::SIZE]) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        // as inner array, while array of byte arrays has the same alignment as a single byte
        unsafe { mem::transmute(value) }
    }
}

impl Record {
    /// Number of chunks within one record.
    pub const NUM_CHUNKS: usize = 2_usize.pow(15);
    /// Number of s-buckets contained within one sector record.
    ///
    /// Essentially we chunk records and erasure code them.
    pub const NUM_S_BUCKETS: usize = Record::NUM_CHUNKS
        * RecordedHistorySegment::ERASURE_CODING_RATE.1
        / RecordedHistorySegment::ERASURE_CODING_RATE.0;
    /// Size of a segment record, it is guaranteed to be a multiple of [`ScalarBytes::FULL_BYTES`]
    pub const SIZE: usize = ScalarBytes::FULL_BYTES * Record::NUM_CHUNKS;

    /// Create boxed value without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        // SAFETY: Data structure filled with zeroes is a valid invariant
        unsafe { Box::new_zeroed().assume_init() }
    }

    /// Create vector filled with zeroed records without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_zero_vec(length: usize) -> Vec<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        let mut records = Vec::with_capacity(length);
        {
            let slice = records.spare_capacity_mut();
            // SAFETY: Same memory layout due to `#[repr(transparent)]` on `Record` and
            // `MaybeUninit<[[T; M]; N]>` is guaranteed to have the same layout as
            // `[[MaybeUninit<T>; M]; N]`
            let slice = unsafe {
                slice::from_raw_parts_mut(
                    slice.as_mut_ptr()
                        as *mut [[mem::MaybeUninit<u8>; ScalarBytes::FULL_BYTES];
                            Record::NUM_CHUNKS],
                    length,
                )
            };
            for byte in slice.as_flattened_mut().as_flattened_mut() {
                byte.write(0);
            }
        }
        // SAFETY: All values are initialized above.
        unsafe {
            records.set_len(records.capacity());
        }

        records
    }

    /// Convenient conversion from slice of record to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn slice_to_repr(value: &[Self]) -> &[[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to record for efficiency
    /// purposes.
    #[inline]
    pub fn slice_from_repr(
        value: &[[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]],
    ) -> &[Self] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of record to underlying representation for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_to_repr(
        value: &mut [Self],
    ) -> &mut [[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to record for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_from_repr(
        value: &mut [[[u8; ScalarBytes::FULL_BYTES]; Record::NUM_CHUNKS]],
    ) -> &mut [Self] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Record commitment contained within a piece.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct RecordCommitment([u8; RecordCommitment::SIZE]);

impl fmt::Debug for RecordCommitment {
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
struct RecordCommitmentBinary(#[serde(with = "BigArray")] [u8; RecordCommitment::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordCommitmentHex(#[serde(with = "hex")] [u8; RecordCommitment::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for RecordCommitment {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RecordCommitmentHex(self.0).serialize(serializer)
        } else {
            RecordCommitmentBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RecordCommitment {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RecordCommitmentHex::deserialize(deserializer)?.0
        } else {
            RecordCommitmentBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for RecordCommitment {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl TryFrom<&[u8]> for RecordCommitment {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <[u8; Self::SIZE]>::try_from(slice).map(Self)
    }
}

impl AsRef<[u8]> for RecordCommitment {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for RecordCommitment {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&RecordCommitment> for &[u8; RecordCommitment::SIZE] {
    #[inline]
    fn from(value: &RecordCommitment) -> Self {
        // SAFETY: `RecordCommitment` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordCommitment::SIZE]> for &RecordCommitment {
    #[inline]
    fn from(value: &[u8; RecordCommitment::SIZE]) -> Self {
        // SAFETY: `RecordCommitment` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordCommitment> for &mut [u8; RecordCommitment::SIZE] {
    #[inline]
    fn from(value: &mut RecordCommitment) -> Self {
        // SAFETY: `RecordCommitment` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordCommitment::SIZE]> for &mut RecordCommitment {
    #[inline]
    fn from(value: &mut [u8; RecordCommitment::SIZE]) -> Self {
        // SAFETY: `RecordCommitment` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordCommitment {
    /// Size of record commitment in bytes.
    pub const SIZE: usize = 32;
}

/// Record chunks root (source or parity) contained within a piece.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct RecordChunksRoot([u8; RecordChunksRoot::SIZE]);

impl fmt::Debug for RecordChunksRoot {
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
struct RecordChunksRootBinary(#[serde(with = "BigArray")] [u8; RecordChunksRoot::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordChunksRootHex(#[serde(with = "hex")] [u8; RecordChunksRoot::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for RecordChunksRoot {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RecordChunksRootHex(self.0).serialize(serializer)
        } else {
            RecordChunksRootBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RecordChunksRoot {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RecordChunksRootHex::deserialize(deserializer)?.0
        } else {
            RecordChunksRootBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for RecordChunksRoot {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl TryFrom<&[u8]> for RecordChunksRoot {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <[u8; Self::SIZE]>::try_from(slice).map(Self)
    }
}

impl AsRef<[u8]> for RecordChunksRoot {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for RecordChunksRoot {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&RecordChunksRoot> for &[u8; RecordChunksRoot::SIZE] {
    #[inline]
    fn from(value: &RecordChunksRoot) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordChunksRoot::SIZE]> for &RecordChunksRoot {
    #[inline]
    fn from(value: &[u8; RecordChunksRoot::SIZE]) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordChunksRoot> for &mut [u8; RecordChunksRoot::SIZE] {
    #[inline]
    fn from(value: &mut RecordChunksRoot) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordChunksRoot::SIZE]> for &mut RecordChunksRoot {
    #[inline]
    fn from(value: &mut [u8; RecordChunksRoot::SIZE]) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordChunksRoot {
    /// Size of record chunks root in bytes.
    pub const SIZE: usize = 32;
}

// TODO: Change commitment/witness terminology to root/proof
/// Record witness contained within a piece.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
pub struct RecordWitness([u8; RecordWitness::SIZE]);

impl fmt::Debug for RecordWitness {
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
struct RecordWitnessBinary(#[serde(with = "BigArray")] [u8; RecordWitness::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordWitnessHex(#[serde(with = "hex")] [u8; RecordWitness::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for RecordWitness {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RecordWitnessHex(self.0).serialize(serializer)
        } else {
            RecordWitnessBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RecordWitness {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RecordWitnessHex::deserialize(deserializer)?.0
        } else {
            RecordWitnessBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for RecordWitness {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl TryFrom<&[u8]> for RecordWitness {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <[u8; Self::SIZE]>::try_from(slice).map(Self)
    }
}

impl AsRef<[u8]> for RecordWitness {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for RecordWitness {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&RecordWitness> for &[u8; RecordWitness::SIZE] {
    #[inline]
    fn from(value: &RecordWitness) -> Self {
        // SAFETY: `RecordWitness` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordWitness::SIZE]> for &RecordWitness {
    #[inline]
    fn from(value: &[u8; RecordWitness::SIZE]) -> Self {
        // SAFETY: `RecordWitness` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordWitness> for &mut [u8; RecordWitness::SIZE] {
    #[inline]
    fn from(value: &mut RecordWitness) -> Self {
        // SAFETY: `RecordWitness` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordWitness::SIZE]> for &mut RecordWitness {
    #[inline]
    fn from(value: &mut [u8; RecordWitness::SIZE]) -> Self {
        // SAFETY: `RecordWitness` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordWitness {
    /// Size of record witness in bytes.
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = RecordedHistorySegment::NUM_PIECES.ilog2() as usize;
}

/// A piece of archival history in Subspace Network.
///
/// This version is allocated on the stack, for heap-allocated piece see [`Piece`].
///
/// Internally a piece contains a record, followed by record commitment, supplementary record chunk
/// root and a witness proving this piece belongs to can be used to verify that a piece belongs to
/// the actual archival history of the blockchain.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deref, DerefMut, AsRef, AsMut)]
#[repr(transparent)]
pub struct PieceArray([u8; PieceArray::SIZE]);

impl fmt::Debug for PieceArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Default for PieceArray {
    #[inline]
    fn default() -> Self {
        Self([0u8; Self::SIZE])
    }
}

impl AsRef<[u8]> for PieceArray {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for PieceArray {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&PieceArray> for &[u8; PieceArray::SIZE] {
    #[inline]
    fn from(value: &PieceArray) -> Self {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; PieceArray::SIZE]> for &PieceArray {
    #[inline]
    fn from(value: &[u8; PieceArray::SIZE]) -> Self {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut PieceArray> for &mut [u8; PieceArray::SIZE] {
    #[inline]
    fn from(value: &mut PieceArray) -> Self {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; PieceArray::SIZE]> for &mut PieceArray {
    #[inline]
    fn from(value: &mut [u8; PieceArray::SIZE]) -> Self {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

impl PieceArray {
    /// Size of a piece (in bytes).
    pub const SIZE: usize =
        Record::SIZE + RecordCommitment::SIZE + RecordChunksRoot::SIZE + RecordWitness::SIZE;

    /// Create boxed value without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        // SAFETY: Data structure filled with zeroes is a valid invariant
        unsafe { Box::<Self>::new_zeroed().assume_init() }
    }

    /// Split piece into underlying components.
    #[inline]
    pub fn split(
        &self,
    ) -> (
        &Record,
        &RecordCommitment,
        &RecordChunksRoot,
        &RecordWitness,
    ) {
        let (record, extra) = self.0.split_at(Record::SIZE);
        let (commitment, extra) = extra.split_at(RecordCommitment::SIZE);
        let (parity_chunks_root, witness) = extra.split_at(RecordChunksRoot::SIZE);

        let record = <&[u8; Record::SIZE]>::try_from(record)
            .expect("Slice of memory has correct length; qed");
        let commitment = <&[u8; RecordCommitment::SIZE]>::try_from(commitment)
            .expect("Slice of memory has correct length; qed");
        let parity_chunks_root = <&[u8; RecordChunksRoot::SIZE]>::try_from(parity_chunks_root)
            .expect("Slice of memory has correct length; qed");
        let witness = <&[u8; RecordWitness::SIZE]>::try_from(witness)
            .expect("Slice of memory has correct length; qed");

        (
            record.into(),
            commitment.into(),
            parity_chunks_root.into(),
            witness.into(),
        )
    }

    /// Split piece into underlying mutable components.
    #[inline]
    pub fn split_mut(
        &mut self,
    ) -> (
        &mut Record,
        &mut RecordCommitment,
        &mut RecordChunksRoot,
        &mut RecordWitness,
    ) {
        let (record, extra) = self.0.split_at_mut(Record::SIZE);
        let (commitment, extra) = extra.split_at_mut(RecordCommitment::SIZE);
        let (parity_chunks_root, witness) = extra.split_at_mut(RecordChunksRoot::SIZE);

        let record = <&mut [u8; Record::SIZE]>::try_from(record)
            .expect("Slice of memory has correct length; qed");
        let commitment = <&mut [u8; RecordCommitment::SIZE]>::try_from(commitment)
            .expect("Slice of memory has correct length; qed");
        let parity_chunks_root = <&mut [u8; RecordChunksRoot::SIZE]>::try_from(parity_chunks_root)
            .expect("Slice of memory has correct length; qed");
        let witness = <&mut [u8; RecordWitness::SIZE]>::try_from(witness)
            .expect("Slice of memory has correct length; qed");

        (
            record.into(),
            commitment.into(),
            parity_chunks_root.into(),
            witness.into(),
        )
    }

    /// Record contained within a piece.
    #[inline]
    pub fn record(&self) -> &Record {
        self.split().0
    }

    /// Mutable record contained within a piece.
    #[inline]
    pub fn record_mut(&mut self) -> &mut Record {
        self.split_mut().0
    }

    /// Commitment contained within a piece.
    #[inline]
    pub fn commitment(&self) -> &RecordCommitment {
        self.split().1
    }

    /// Mutable commitment contained within a piece.
    #[inline]
    pub fn commitment_mut(&mut self) -> &mut RecordCommitment {
        self.split_mut().1
    }

    /// Parity chunks root contained within a piece.
    #[inline]
    pub fn parity_chunks_root(&self) -> &RecordChunksRoot {
        self.split().2
    }

    /// Mutable parity chunks root contained within a piece.
    #[inline]
    pub fn parity_chunks_root_mut(&mut self) -> &mut RecordChunksRoot {
        self.split_mut().2
    }

    /// Witness contained within a piece.
    #[inline]
    pub fn witness(&self) -> &RecordWitness {
        self.split().3
    }

    /// Mutable witness contained within a piece.
    #[inline]
    pub fn witness_mut(&mut self) -> &mut RecordWitness {
        self.split_mut().3
    }

    /// Convenient conversion from slice of piece array to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to piece array for efficiency
    /// purposes.
    #[inline]
    pub fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of piece array to underlying representation for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_to_repr(value: &mut [Self]) -> &mut [[u8; Self::SIZE]] {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to piece array for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_from_repr(value: &mut [[u8; Self::SIZE]]) -> &mut [Self] {
        // SAFETY: `PieceArray` is `#[repr(transparent)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}

#[cfg(feature = "alloc")]
impl From<Box<PieceArray>> for Vec<u8> {
    fn from(value: Box<PieceArray>) -> Self {
        let mut value = mem::ManuallyDrop::new(value);
        // SAFETY: Always contains fixed allocation of bytes
        unsafe { Vec::from_raw_parts(value.as_mut_ptr(), PieceArray::SIZE, PieceArray::SIZE) }
    }
}
