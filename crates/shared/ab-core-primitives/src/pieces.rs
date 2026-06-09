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
use crate::segments::{
    LocalSegmentIndex, RecordedHistorySegment, SegmentIndex, SegmentPosition, SegmentRoot,
    SuperSegmentIndex, SuperSegmentRoot,
};
use crate::shard::ShardIndex;
#[cfg(feature = "serde")]
use ::serde::{Deserialize, Deserializer, Serialize, Serializer};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_merkle_tree::balanced::BalancedMerkleTree;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use blake3::OUT_LEN;
use core::array::TryFromSliceError;
use core::hash::Hash;
use core::iter::Step;
use core::mem::MaybeUninit;
#[cfg(feature = "alloc")]
use core::slice;
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, AsMut, AsRef, Deref, DerefMut, Display, Div, DivAssign, From, Into, Mul,
    MulAssign, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Piece index
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
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
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

impl const From<u64> for PieceIndex {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl const From<PieceIndex> for u64 {
    #[inline(always)]
    fn from(value: PieceIndex) -> Self {
        value.0
    }
}

impl PieceIndex {
    /// Size in bytes.
    pub const SIZE: usize = size_of::<u64>();
    /// Piece index 0.
    pub const ZERO: PieceIndex = PieceIndex(0);
    /// Piece index 1.
    pub const ONE: PieceIndex = PieceIndex(1);

    /// Create a piece index from bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert a piece index to bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }

    /// Segment index piece index corresponds to
    #[inline]
    pub const fn segment_index(&self) -> SegmentIndex {
        SegmentIndex::from(self.0 / RecordedHistorySegment::NUM_PIECES as u64)
    }

    /// Position of a piece in a segment
    #[inline]
    pub fn position(&self) -> PiecePosition {
        PiecePosition::from((self.0 % RecordedHistorySegment::NUM_PIECES as u64) as u8)
    }
}

const {
    // Assert that `u8` represents `PiecePosition` perfectly
    assert!(RecordedHistorySegment::NUM_PIECES == usize::from(u8::MAX) + 1);
}

/// Piece position in a segment
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
    TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct PiecePosition(u8);

impl Step for PiecePosition {
    #[inline]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u8::steps_between(&start.0, &end.0)
    }

    #[inline]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u8::forward_checked(start.0, count).map(Self)
    }

    #[inline]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u8::backward_checked(start.0, count).map(Self)
    }
}

impl From<PiecePosition> for u16 {
    #[inline]
    fn from(original: PiecePosition) -> Self {
        Self::from(original.0)
    }
}

impl From<PiecePosition> for u32 {
    #[inline]
    fn from(original: PiecePosition) -> Self {
        Self::from(original.0)
    }
}

impl From<PiecePosition> for u64 {
    #[inline]
    fn from(original: PiecePosition) -> Self {
        Self::from(original.0)
    }
}

impl From<PiecePosition> for usize {
    #[inline]
    fn from(original: PiecePosition) -> Self {
        usize::from(original.0)
    }
}

/// Piece offset in a sector
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
    /// Piece index 0
    pub const ZERO: Self = Self(0);
    /// Piece index 1
    pub const ONE: Self = Self(1);
    /// Size in bytes
    pub const SIZE: usize = size_of::<u16>();

    /// Convert piece offset to bytes
    #[inline]
    pub const fn to_bytes(self) -> [u8; size_of::<u16>()] {
        self.0.to_le_bytes()
    }
}

/// Chunk contained in a record
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
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(C)]
pub struct RecordChunk([u8; RecordChunk::SIZE]);

impl fmt::Debug for RecordChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl RecordChunk {
    /// Size of the chunk in bytes
    pub const SIZE: usize = 32;

    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; RecordChunk::SIZE]] {
        // SAFETY: `RecordChunk` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline]
    pub fn slice_from_repr(value: &[[u8; RecordChunk::SIZE]]) -> &[Self] {
        // SAFETY: `RecordChunk` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline]
    pub fn slice_mut_to_repr(value: &mut [Self]) -> &mut [[u8; RecordChunk::SIZE]] {
        // SAFETY: `RecordChunk` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation for efficiency
    /// purposes
    #[inline]
    pub fn slice_mut_from_repr(value: &mut [[u8; RecordChunk::SIZE]]) -> &mut [Self] {
        // SAFETY: `RecordChunk` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Record contained within a piece.
///
/// NOTE: This is a stack-allocated data structure and can cause stack overflow!
#[derive(Copy, Clone, Eq, PartialEq, Deref, DerefMut, TrivialType)]
#[repr(C)]
pub struct Record([[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]);

impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_flattened() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
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

impl From<&Record> for &[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &Record) -> Self {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]> for &Record {
    #[inline]
    fn from(value: &[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]) -> Self {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut Record> for &mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &mut Record) -> Self {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]> for &mut Record {
    #[inline]
    fn from(value: &mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]) -> Self {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl Record {
    /// Number of chunks within one record.
    pub const NUM_CHUNKS: usize = 2usize.pow(15);
    /// Number of s-buckets contained within one sector record.
    ///
    /// Essentially we chunk records and erasure code them.
    pub const NUM_S_BUCKETS: usize = Record::NUM_CHUNKS
        * RecordedHistorySegment::ERASURE_CODING_RATE.1
        / RecordedHistorySegment::ERASURE_CODING_RATE.0;
    /// Size of a segment record, it is guaranteed to be a multiple of [`RecordChunk::SIZE`]
    pub const SIZE: usize = RecordChunk::SIZE * Record::NUM_CHUNKS;

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
        // TODO: Should have been just `vec![Self::default(); length]`, but
        //  https://github.com/rust-lang/rust/issues/53827
        let mut records = Vec::with_capacity(length);
        {
            let slice = records.spare_capacity_mut();
            // SAFETY: Same memory layout due to `#[repr(C)]` on `Record` and
            // `MaybeUninit<[[T; M]; N]>` is guaranteed to have the same layout as
            // `[[MaybeUninit<T>; M]; N]`
            let slice = unsafe {
                slice::from_raw_parts_mut(
                    slice
                        .as_mut_ptr()
                        .cast::<[[mem::MaybeUninit<u8>; RecordChunk::SIZE]; Record::NUM_CHUNKS]>(),
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
    #[inline(always)]
    pub fn slice_to_repr(value: &[Self]) -> &[[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to record for efficiency
    /// purposes.
    #[inline(always)]
    pub fn slice_from_repr(value: &[[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]]) -> &[Self] {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of record to underlying representation for
    /// efficiency purposes.
    #[inline(always)]
    pub fn slice_mut_to_repr(
        value: &mut [Self],
    ) -> &mut [[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to record for
    /// efficiency purposes.
    #[inline(always)]
    pub fn slice_mut_from_repr(
        value: &mut [[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]],
    ) -> &mut [Self] {
        // SAFETY: `Record` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Derive source chunks root on-demand
    #[inline(always)]
    pub fn source_chunks_root(&self) -> RecordChunksRoot {
        RecordChunksRoot(BalancedMerkleTree::compute_root_only(self))
    }
}

/// Root of the record contained within a piece.
///
/// This is a Merkle Tree root of the roots of source and parity record chunks.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct RecordRoot([u8; RecordRoot::SIZE]);

impl fmt::Debug for RecordRoot {
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
struct RecordRootBinary(#[serde(with = "BigArray")] [u8; RecordRoot::SIZE]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordRootHex(#[serde(with = "hex")] [u8; RecordRoot::SIZE]);

#[cfg(feature = "serde")]
impl Serialize for RecordRoot {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            RecordRootHex(self.0).serialize(serializer)
        } else {
            RecordRootBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RecordRoot {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            RecordRootHex::deserialize(deserializer)?.0
        } else {
            RecordRootBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for RecordRoot {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
    }
}

impl TryFrom<&[u8]> for RecordRoot {
    type Error = TryFromSliceError;

    #[inline]
    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        <[u8; Self::SIZE]>::try_from(slice).map(Self)
    }
}

impl AsRef<[u8]> for RecordRoot {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for RecordRoot {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&RecordRoot> for &[u8; RecordRoot::SIZE] {
    #[inline]
    fn from(value: &RecordRoot) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordRoot::SIZE]> for &RecordRoot {
    #[inline]
    fn from(value: &[u8; RecordRoot::SIZE]) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordRoot> for &mut [u8; RecordRoot::SIZE] {
    #[inline]
    fn from(value: &mut RecordRoot) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordRoot::SIZE]> for &mut RecordRoot {
    #[inline]
    fn from(value: &mut [u8; RecordRoot::SIZE]) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordRoot {
    /// Size of record root in bytes.
    pub const SIZE: usize = 32;

    /// Validate record root hash produced by the archiver
    pub fn is_valid(
        &self,
        segment_root: &SegmentRoot,
        record_proof: &RecordProof,
        position: PiecePosition,
    ) -> bool {
        BalancedMerkleTree::<{ RecordedHistorySegment::NUM_PIECES }>::verify(
            segment_root,
            record_proof,
            usize::from(position),
            self.0,
        )
    }
}

/// Root of source or parity record chunks
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
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
        // SAFETY: `RecordChunksRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordChunksRoot::SIZE]> for &RecordChunksRoot {
    #[inline]
    fn from(value: &[u8; RecordChunksRoot::SIZE]) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordChunksRoot> for &mut [u8; RecordChunksRoot::SIZE] {
    #[inline]
    fn from(value: &mut RecordChunksRoot) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordChunksRoot::SIZE]> for &mut RecordChunksRoot {
    #[inline]
    fn from(value: &mut [u8; RecordChunksRoot::SIZE]) -> Self {
        // SAFETY: `RecordChunksRoot` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordChunksRoot {
    /// Size of record chunks root in bytes.
    pub const SIZE: usize = 32;
}

/// Proof that the record (root) belongs to a segment
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct RecordProof([[u8; OUT_LEN]; RecordProof::NUM_HASHES]);

impl fmt::Debug for RecordProof {
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
struct RecordProofBinary([[u8; OUT_LEN]; RecordProof::NUM_HASHES]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordProofHexHash(#[serde(with = "hex")] [u8; OUT_LEN]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct RecordProofHex([RecordProofHexHash; RecordProof::NUM_HASHES]);

#[cfg(feature = "serde")]
impl Serialize for RecordProof {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // SAFETY: `RecordProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            RecordProofHex(unsafe {
                mem::transmute::<
                    [[u8; OUT_LEN]; RecordProof::NUM_HASHES],
                    [RecordProofHexHash; RecordProof::NUM_HASHES],
                >(self.0)
            })
            .serialize(serializer)
        } else {
            RecordProofBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for RecordProof {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            // SAFETY: `RecordProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            unsafe {
                mem::transmute::<
                    [RecordProofHexHash; RecordProof::NUM_HASHES],
                    [[u8; OUT_LEN]; RecordProof::NUM_HASHES],
                >(RecordProofHex::deserialize(deserializer)?.0)
            }
        } else {
            RecordProofBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for RecordProof {
    #[inline]
    fn default() -> Self {
        Self([[0; OUT_LEN]; RecordProof::NUM_HASHES])
    }
}

impl AsRef<[u8]> for RecordProof {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_flattened()
    }
}

impl AsMut<[u8]> for RecordProof {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_flattened_mut()
    }
}

impl From<&RecordProof> for &[u8; RecordProof::SIZE] {
    #[inline]
    fn from(value: &RecordProof) -> Self {
        // SAFETY: `RecordProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordProof::SIZE]> for &RecordProof {
    #[inline]
    fn from(value: &[u8; RecordProof::SIZE]) -> Self {
        // SAFETY: `RecordProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordProof> for &mut [u8; RecordProof::SIZE] {
    #[inline]
    fn from(value: &mut RecordProof) -> Self {
        // SAFETY: `RecordProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordProof::SIZE]> for &mut RecordProof {
    #[inline]
    fn from(value: &mut [u8; RecordProof::SIZE]) -> Self {
        // SAFETY: `RecordProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordProof {
    /// Size of record proof in bytes
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = RecordedHistorySegment::NUM_PIECES.ilog2() as usize;
}

/// Proof that the segment belongs to a super segment
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct SegmentProof([[u8; OUT_LEN]; SegmentProof::NUM_HASHES]);

impl fmt::Debug for SegmentProof {
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
struct SegmentProofBinary([[u8; OUT_LEN]; SegmentProof::NUM_HASHES]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SegmentProofHexHash(#[serde(with = "hex")] [u8; OUT_LEN]);

#[cfg(feature = "serde")]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SegmentProofHex([SegmentProofHexHash; SegmentProof::NUM_HASHES]);

#[cfg(feature = "serde")]
impl Serialize for SegmentProof {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            // SAFETY: `SegmentProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            SegmentProofHex(unsafe {
                mem::transmute::<
                    [[u8; OUT_LEN]; SegmentProof::NUM_HASHES],
                    [SegmentProofHexHash; SegmentProof::NUM_HASHES],
                >(self.0)
            })
            .serialize(serializer)
        } else {
            SegmentProofBinary(self.0).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for SegmentProof {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(if deserializer.is_human_readable() {
            // SAFETY: `SegmentProofHexHash` is `#[repr(C)]` and guaranteed to have the
            // same memory layout
            unsafe {
                mem::transmute::<
                    [SegmentProofHexHash; SegmentProof::NUM_HASHES],
                    [[u8; OUT_LEN]; SegmentProof::NUM_HASHES],
                >(SegmentProofHex::deserialize(deserializer)?.0)
            }
        } else {
            SegmentProofBinary::deserialize(deserializer)?.0
        }))
    }
}

impl Default for SegmentProof {
    #[inline]
    fn default() -> Self {
        Self([[0; OUT_LEN]; SegmentProof::NUM_HASHES])
    }
}

impl AsRef<[u8]> for SegmentProof {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.0.as_flattened()
    }
}

impl AsMut<[u8]> for SegmentProof {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_flattened_mut()
    }
}

impl From<&SegmentProof> for &[u8; SegmentProof::SIZE] {
    #[inline]
    fn from(value: &SegmentProof) -> Self {
        // SAFETY: `SegmentProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; SegmentProof::SIZE]> for &SegmentProof {
    #[inline]
    fn from(value: &[u8; SegmentProof::SIZE]) -> Self {
        // SAFETY: `SegmentProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut SegmentProof> for &mut [u8; SegmentProof::SIZE] {
    #[inline]
    fn from(value: &mut SegmentProof) -> Self {
        // SAFETY: `SegmentProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; SegmentProof::SIZE]> for &mut SegmentProof {
    #[inline]
    fn from(value: &mut [u8; SegmentProof::SIZE]) -> Self {
        // SAFETY: `SegmentProof` is `#[repr(C)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl SegmentProof {
    /// Size of segment proof in bytes
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = SuperSegmentRoot::MAX_SEGMENTS.next_power_of_two().ilog2() as usize;

    /// Returns a mutable reference to an internal array as uninitialized memory.
    ///
    /// This is a convenience method for proof generation.
    pub fn as_uninit_repr(
        &mut self,
    ) -> &mut [MaybeUninit<[u8; OUT_LEN]>; SegmentProof::NUM_HASHES] {
        // SAFETY: Casting initialized memory into uninitialized memory of the same size is safe
        unsafe { mem::transmute(&mut self.0) }
    }
}

/// Header for a piece of archival history.
///
/// Primarily contains information needed for piece verification.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct PieceHeader {
    /// Shard index
    pub shard_index: Unaligned<ShardIndex>,
    /// Local segment index
    pub local_segment_index: Unaligned<LocalSegmentIndex>,
    /// Super segment index
    pub super_segment_index: Unaligned<SuperSegmentIndex>,
    /// Position of the segment in the super segment
    pub segment_position: Unaligned<SegmentPosition>,
    /// Segment root
    pub segment_root: SegmentRoot,
    /// Segment proof
    pub segment_proof: SegmentProof,
    /// Root of parity record chunks.
    ///
    /// Technically redundant, but helps to avoid repeating erasure coding during verification.
    pub parity_chunks_root: RecordChunksRoot,
    /// Proof that the record (root) belongs to a segment
    pub record_proof: RecordProof,
}

const {
    // Must have minimal alignment for various conversions to/from bytes
    assert!(align_of::<PieceHeader>() == 1);
}

/// A piece of archival history.
///
/// This version is allocated on the stack, for a heap-allocated piece that can be moved around
/// efficiently, see [`Piece`].
///
/// Internally, a piece contains a record, supplementary record chunk root, and a proof proving this
/// piece belongs to can be used to verify that a piece belongs to the actual archival history of
/// the blockchain.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(C)]
pub struct InnerPiece {
    /// Piece header
    pub header: PieceHeader,
    /// Record contained within a piece
    pub record: Record,
}

const {
    // Must have minimal alignment for various conversions to/from bytes
    assert!(align_of::<InnerPiece>() == 1);
}

impl InnerPiece {
    /// Size of a piece (in bytes)
    pub const SIZE: usize = size_of::<Self>();

    /// Create boxed value without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        // SAFETY: Data structure filled with zeroes is a valid invariant
        unsafe { Box::<Self>::new_zeroed().assume_init() }
    }

    /// Check whether the piece is valid against the matching super segment root
    pub fn is_valid(
        &self,
        super_segment_root: &SuperSegmentRoot,
        num_segments: u32,
        position: PiecePosition,
    ) -> bool {
        if !self.header.segment_root.is_valid(
            self.header.shard_index.as_inner(),
            self.header.local_segment_index.as_inner(),
            self.header.segment_position.as_inner(),
            &self.header.segment_proof,
            num_segments,
            super_segment_root,
        ) {
            return false;
        }
        self.record_root().is_valid(
            &self.header.segment_root,
            &self.header.record_proof,
            position,
        )
    }

    /// Root of the record contained within a piece.
    ///
    /// It is re-derived on every call of this function.
    #[inline]
    pub fn record_root(&self) -> RecordRoot {
        let record_merkle_tree_root = BalancedMerkleTree::compute_root_only(&[
            *self.record.source_chunks_root(),
            *self.header.parity_chunks_root,
        ]);

        RecordRoot::from(record_merkle_tree_root)
    }

    /// Convenient conversion from slice of piece array to underlying representation for efficiency
    /// purposes.
    #[inline]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `PieceArray` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to piece array for efficiency
    /// purposes.
    #[inline]
    pub fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `PieceArray` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of piece array to underlying representation for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_to_repr(value: &mut [Self]) -> &mut [[u8; Self::SIZE]] {
        // SAFETY: `PieceArray` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to piece array for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_from_repr(value: &mut [[u8; Self::SIZE]]) -> &mut [Self] {
        // SAFETY: `PieceArray` is `#[repr(C)]` and guaranteed to have the same memory
        // layout
        unsafe { mem::transmute(value) }
    }
}
