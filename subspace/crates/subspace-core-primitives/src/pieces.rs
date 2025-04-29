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
use crate::segments::{RecordedHistorySegment, SegmentIndex, SegmentRoot};
#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use ::serde::{Deserializer, Serializer};
use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
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
    pub const SIZE: usize = size_of::<u64>();
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

    /// Is this piece index a source piece?
    #[inline]
    pub const fn is_source(&self) -> bool {
        self.position() < RecordedHistorySegment::NUM_RAW_RECORDS as u32
    }

    /// Returns the next source piece index.
    /// Panics if the piece is not a source piece.
    #[inline]
    pub const fn next_source_index(&self) -> Self {
        if self.position() + 1 < RecordedHistorySegment::NUM_RAW_RECORDS as u32 {
            // Same segment
            Self(self.0 + 1)
        } else {
            // Next segment
            Self(self.0 + RecordedHistorySegment::NUM_RAW_RECORDS as u64)
        }
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
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, TypeInfo))]
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
#[derive(Copy, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(transparent)]
pub struct Record([[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]);

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

impl From<&Record> for &[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]> for &Record {
    #[inline]
    fn from(value: &[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut Record> for &mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS] {
    #[inline]
    fn from(value: &mut Record) -> Self {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]> for &mut Record {
    #[inline]
    fn from(value: &mut [[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]) -> Self {
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
                        as *mut [[mem::MaybeUninit<u8>; RecordChunk::SIZE]; Record::NUM_CHUNKS],
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
    pub fn slice_to_repr(value: &[Self]) -> &[[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation to record for efficiency
    /// purposes.
    #[inline]
    pub fn slice_from_repr(value: &[[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]]) -> &[Self] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of record to underlying representation for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_to_repr(
        value: &mut [Self],
    ) -> &mut [[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from mutable slice of underlying representation to record for
    /// efficiency purposes.
    #[inline]
    pub fn slice_mut_from_repr(
        value: &mut [[[u8; RecordChunk::SIZE]; Record::NUM_CHUNKS]],
    ) -> &mut [Self] {
        // SAFETY: `Record` is `#[repr(transparent)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Record root contained within a piece.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
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
        // SAFETY: `RecordRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordRoot::SIZE]> for &RecordRoot {
    #[inline]
    fn from(value: &[u8; RecordRoot::SIZE]) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordRoot> for &mut [u8; RecordRoot::SIZE] {
    #[inline]
    fn from(value: &mut RecordRoot) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordRoot::SIZE]> for &mut RecordRoot {
    #[inline]
    fn from(value: &mut [u8; RecordRoot::SIZE]) -> Self {
        // SAFETY: `RecordRoot` is `#[repr(transparent)]` and guaranteed to have the same
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
        position: u32,
    ) -> bool {
        BalancedHashedMerkleTree::<{ RecordedHistorySegment::NUM_PIECES }>::verify(
            segment_root,
            record_proof,
            position as usize,
            self.0,
        )
    }
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

/// Record proof contained within a piece.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
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
            // SAFETY: `RecordProofHexHash` is `#[repr(transparent)]` and guaranteed to have the
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
            // SAFETY: `RecordProofHexHash` is `#[repr(transparent)]` and guaranteed to have the
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
        // SAFETY: `RecordProof` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&[u8; RecordProof::SIZE]> for &RecordProof {
    #[inline]
    fn from(value: &[u8; RecordProof::SIZE]) -> Self {
        // SAFETY: `RecordProof` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut RecordProof> for &mut [u8; RecordProof::SIZE] {
    #[inline]
    fn from(value: &mut RecordProof) -> Self {
        // SAFETY: `RecordProof` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl From<&mut [u8; RecordProof::SIZE]> for &mut RecordProof {
    #[inline]
    fn from(value: &mut [u8; RecordProof::SIZE]) -> Self {
        // SAFETY: `RecordProof` is `#[repr(transparent)]` and guaranteed to have the same
        // memory layout
        unsafe { mem::transmute(value) }
    }
}

impl RecordProof {
    /// Size of record proof in bytes.
    pub const SIZE: usize = OUT_LEN * Self::NUM_HASHES;
    const NUM_HASHES: usize = RecordedHistorySegment::NUM_PIECES.ilog2() as usize;
}

/// A piece of archival history in Subspace Network.
///
/// This version is allocated on the stack, for heap-allocated piece see [`Piece`].
///
/// Internally a piece contains a record, followed by record root, supplementary record chunk
/// root and a proof proving this piece belongs to can be used to verify that a piece belongs to
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
        Record::SIZE + RecordRoot::SIZE + RecordChunksRoot::SIZE + RecordProof::SIZE;

    /// Create boxed value without hitting stack overflow
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
        // SAFETY: Data structure filled with zeroes is a valid invariant
        unsafe { Box::<Self>::new_zeroed().assume_init() }
    }

    /// Validate proof embedded within a piece produced by the archiver
    pub fn is_valid(&self, segment_root: &SegmentRoot, position: u32) -> bool {
        let (record, &record_root, parity_chunks_root, record_proof) = self.split();

        let source_record_merkle_tree_root = BalancedHashedMerkleTree::compute_root_only(record);
        let record_merkle_tree_root = BalancedHashedMerkleTree::compute_root_only(&[
            source_record_merkle_tree_root,
            **parity_chunks_root,
        ]);

        if record_merkle_tree_root != *record_root {
            return false;
        }

        record_root.is_valid(segment_root, record_proof, position)
    }

    /// Split piece into underlying components.
    #[inline]
    pub fn split(&self) -> (&Record, &RecordRoot, &RecordChunksRoot, &RecordProof) {
        let (record, extra) = self.0.split_at(Record::SIZE);
        let (root, extra) = extra.split_at(RecordRoot::SIZE);
        let (parity_chunks_root, proof) = extra.split_at(RecordChunksRoot::SIZE);

        let record = <&[u8; Record::SIZE]>::try_from(record)
            .expect("Slice of memory has correct length; qed");
        let root = <&[u8; RecordRoot::SIZE]>::try_from(root)
            .expect("Slice of memory has correct length; qed");
        let parity_chunks_root = <&[u8; RecordChunksRoot::SIZE]>::try_from(parity_chunks_root)
            .expect("Slice of memory has correct length; qed");
        let proof = <&[u8; RecordProof::SIZE]>::try_from(proof)
            .expect("Slice of memory has correct length; qed");

        (
            record.into(),
            root.into(),
            parity_chunks_root.into(),
            proof.into(),
        )
    }

    /// Split piece into underlying mutable components.
    #[inline]
    pub fn split_mut(
        &mut self,
    ) -> (
        &mut Record,
        &mut RecordRoot,
        &mut RecordChunksRoot,
        &mut RecordProof,
    ) {
        let (record, extra) = self.0.split_at_mut(Record::SIZE);
        let (root, extra) = extra.split_at_mut(RecordRoot::SIZE);
        let (parity_chunks_root, proof) = extra.split_at_mut(RecordChunksRoot::SIZE);

        let record = <&mut [u8; Record::SIZE]>::try_from(record)
            .expect("Slice of memory has correct length; qed");
        let root = <&mut [u8; RecordRoot::SIZE]>::try_from(root)
            .expect("Slice of memory has correct length; qed");
        let parity_chunks_root = <&mut [u8; RecordChunksRoot::SIZE]>::try_from(parity_chunks_root)
            .expect("Slice of memory has correct length; qed");
        let proof = <&mut [u8; RecordProof::SIZE]>::try_from(proof)
            .expect("Slice of memory has correct length; qed");

        (
            record.into(),
            root.into(),
            parity_chunks_root.into(),
            proof.into(),
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

    /// Root contained within a piece.
    #[inline]
    pub fn root(&self) -> &RecordRoot {
        self.split().1
    }

    /// Mutable root contained within a piece.
    #[inline]
    pub fn root_mut(&mut self) -> &mut RecordRoot {
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

    /// Proof contained within a piece.
    #[inline]
    pub fn proof(&self) -> &RecordProof {
        self.split().3
    }

    /// Mutable proof contained within a piece.
    #[inline]
    pub fn proof_mut(&mut self) -> &mut RecordProof {
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
