//! Segments-related data structures

#[cfg(feature = "alloc")]
mod archival_history_segment;

use crate::block::BlockNumber;
use crate::hashes::Blake3Hash;
use crate::pieces::{PieceIndex, Record, SegmentProof};
#[cfg(feature = "alloc")]
pub use crate::segments::archival_history_segment::ArchivedHistorySegment;
use crate::shard::ShardIndex;
use ab_blake3::{single_block_hash, single_chunk_hash};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::sync::Arc as StdArc;
use blake3::{CHUNK_LEN, OUT_LEN};
use core::iter::Step;
use core::num::{NonZeroU32, NonZeroU64};
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, Deref, DerefMut, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub,
    SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_big_array::BigArray;

/// Super segment index
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
    TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct SuperSegmentIndex(u64);

impl Step for SuperSegmentIndex {
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

impl const From<u64> for SuperSegmentIndex {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl const From<SuperSegmentIndex> for u64 {
    #[inline(always)]
    fn from(value: SuperSegmentIndex) -> Self {
        value.0
    }
}

impl SuperSegmentIndex {
    /// Super segment index 0
    pub const ZERO: Self = Self(0);
    /// Super segment index 1
    pub const ONE: Self = Self(1);

    /// Checked integer subtraction. Computes `self - rhs`, returning `None` if underflow occurred
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Saturating integer subtraction. Computes `self - rhs`, returning zero if underflow
    /// occurred
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Super segment root contained within a beacon chain block
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
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

impl const Default for SuperSegmentRoot {
    #[inline]
    fn default() -> Self {
        Self([0; Self::SIZE])
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
    /// The maximum number of segments in a super segment's Merkle Tree.
    ///
    /// `-1` to minimize the number of bits needed to represent it (exactly 20).
    pub const MAX_SEGMENTS: u32 = 2u32.pow(20) - 1;
}

/// Segment position in a super segment
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
pub struct SegmentPosition(u32);

impl From<SegmentPosition> for u64 {
    #[inline]
    fn from(original: SegmentPosition) -> Self {
        Self::from(original.0)
    }
}

impl SegmentPosition {
    /// Zero position
    pub const ZERO: Self = Self(0);
}

/// Shard segment root with position
#[derive(Debug, Clone, Copy, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct ShardSegmentRootWithPosition {
    /// Shard index
    pub shard_index: ShardIndex,
    /// Position of the segment in the super segment
    pub segment_position: SegmentPosition,
    /// Local segment index
    pub local_segment_index: LocalSegmentIndex,
    /// Segment root
    pub segment_root: SegmentRoot,
}

impl ShardSegmentRootWithPosition {
    /// Hash for super segment creation
    #[inline(always)]
    pub fn hash(&self) -> [u8; OUT_LEN] {
        single_block_hash(self.as_bytes()).expect("Less than a single block worth of bytes; qed")
    }
}

/// Super segment header
#[derive(Debug, Clone, Copy, Eq, PartialEq, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct SuperSegmentHeader {
    /// Super segment index
    pub index: Unaligned<SuperSegmentIndex>,
    /// Super segment root
    pub root: SuperSegmentRoot,
    /// Hash of the previous super segment header
    pub prev_super_segment_header_hash: Blake3Hash,
    /// Max index of the segment in the super segment
    pub max_segment_index: Unaligned<SegmentIndex>,
    /// Target beacon chain block number for the super segment
    pub target_beacon_chain_block_number: Unaligned<BlockNumber>,
    // TODO: New type?
    /// Number of segments in the super segment
    pub num_segments: u32,
}

/// Super segment
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
// TODO: Implement SCALE serialization/deserialization manually (if necessary at all)
// #[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SuperSegment {
    /// Super segment root
    pub header: SuperSegmentHeader,
    /// Segment roots that are included in the super segment
    pub segment_roots: StdArc<[ShardSegmentRootWithPosition]>,
}

#[cfg(feature = "alloc")]
impl SuperSegment {
    /// Create a new instance and derive super segment root.
    ///
    /// Returns `None` if the list of segment roots is empty or there are too many of them.
    pub fn new(
        previous_header: &SuperSegmentHeader,
        target_beacon_chain_block_number: BlockNumber,
        segment_roots: StdArc<[ShardSegmentRootWithPosition]>,
    ) -> Option<Self> {
        let num_segments = u32::try_from(segment_roots.len()).ok()?;
        let max_segment_index = SegmentIndex::from(
            u64::from(previous_header.max_segment_index.as_inner()) + u64::from(num_segments),
        );

        // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that
        //  allows the code to compile. Constant 1048575 is hardcoded here and below for compilation
        //  to succeed.
        const {
            assert!(SuperSegmentRoot::MAX_SEGMENTS == 1048575);
        }
        // TODO: Keyed hash
        let maybe_super_segment_root = UnbalancedMerkleTree::compute_root_only::<1048575, _, _>(
            segment_roots.iter().map(ShardSegmentRootWithPosition::hash),
        )?;

        Some(Self {
            header: SuperSegmentHeader {
                index: (previous_header.index.as_inner() + SuperSegmentIndex::ONE).into(),
                root: SuperSegmentRoot::from(maybe_super_segment_root),
                prev_super_segment_header_hash: Blake3Hash::from(
                    single_chunk_hash(previous_header.as_bytes())
                        .expect("Less than a single chunk worth of bytes; qed"),
                ),
                max_segment_index: max_segment_index.into(),
                target_beacon_chain_block_number: target_beacon_chain_block_number.into(),
                num_segments,
            },
            segment_roots,
        })
    }

    /// Produce a proof for a segment in the super segment at a given position
    pub fn proof_for_segment(&self, segment_position: SegmentPosition) -> Option<SegmentProof> {
        // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that
        //  allows the code to compile. Constant 1048575 is hardcoded here and below for compilation
        //  to succeed.
        const {
            assert!(SuperSegmentRoot::MAX_SEGMENTS == 1048575);
        }
        // TODO: Keyed hash
        let mut segment_proof = SegmentProof::default();
        UnbalancedMerkleTree::compute_root_and_proof_in::<1048575, _, _>(
            self.segment_roots.iter().map(|shard_segment_root| {
                single_block_hash(shard_segment_root.as_bytes())
                    .expect("Less than a single block worth of bytes; qed")
            }),
            u32::from(segment_position) as usize,
            segment_proof.as_uninit_repr(),
        )?;

        Some(segment_proof)
    }
}

/// Local segment index of a shard
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
    TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct LocalSegmentIndex(u64);

impl Step for LocalSegmentIndex {
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

impl const From<u64> for LocalSegmentIndex {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl const From<LocalSegmentIndex> for u64 {
    #[inline(always)]
    fn from(value: LocalSegmentIndex) -> Self {
        value.0
    }
}

impl LocalSegmentIndex {
    /// Local segment index 0
    pub const ZERO: Self = Self(0);
    /// Local segment index 1
    pub const ONE: Self = Self(1);

    /// Checked integer subtraction. Computes `self - rhs`, returning `None` if underflow occurred
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Saturating integer subtraction. Computes `self - rhs`, returning zero if underflow
    /// occurred
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Segment index
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
    TrivialType,
)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
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

impl const From<u64> for SegmentIndex {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl const From<SegmentIndex> for u64 {
    #[inline(always)]
    fn from(value: SegmentIndex) -> Self {
        value.0
    }
}

impl SegmentIndex {
    /// Segment index 0
    pub const ZERO: Self = Self(0);
    /// Segment index 1
    pub const ONE: Self = Self(1);

    /// Get the first piece index in this segment
    #[inline]
    pub const fn first_piece_index(&self) -> PieceIndex {
        PieceIndex::from(self.0 * RecordedHistorySegment::NUM_PIECES as u64)
    }

    /// Get the last piece index in this segment
    #[inline]
    pub const fn last_piece_index(&self) -> PieceIndex {
        PieceIndex::from((self.0 + 1) * RecordedHistorySegment::NUM_PIECES as u64 - 1)
    }

    /// List of piece indexes that belong to this segment
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

    /// Checked integer subtraction. Computes `self - rhs`, returning `None` if underflow occurred
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    /// Saturating integer subtraction. Computes `self - rhs`, returning zero if underflow
    /// occurred
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Segment root contained within a segment
#[derive(Copy, Clone, Eq, PartialEq, Hash, Deref, DerefMut, From, Into, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
#[repr(C)]
pub struct SegmentRoot([u8; SegmentRoot::SIZE]);

impl SegmentRoot {
    /// Check whether a segment root is a part of the super segment
    pub fn is_valid(
        &self,
        shard_index: ShardIndex,
        local_segment_index: LocalSegmentIndex,
        segment_position: SegmentPosition,
        segment_proof: &SegmentProof,
        num_segments: u32,
        super_segment_root: &SuperSegmentRoot,
    ) -> bool {
        let shard_segment_root = ShardSegmentRootWithPosition {
            shard_index,
            segment_position,
            local_segment_index,
            segment_root: *self,
        };
        // The proof is fixed size and contains zero padding elements, which must be skipped for
        // verification purposes
        let segment_proof = segment_proof
            .split_once(|hash| hash == &[0; _])
            .map_or(segment_proof.as_slice(), |(before, _after)| before);
        UnbalancedMerkleTree::verify(
            super_segment_root,
            segment_proof,
            u64::from(segment_position),
            shard_segment_root.hash(),
            u64::from(num_segments),
        )
    }
}

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

    /// Convenient conversion from a slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        // SAFETY: `SegmentRoot` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to a slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `SegmentRoot` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }
}

/// Size of blockchain history in segments
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
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode, MaxEncodedLen))]
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
        Self(SegmentIndex::from(value.get() - 1))
    }

    /// Get internal representation
    pub const fn as_segment_index(&self) -> SegmentIndex {
        self.0
    }

    /// Get internal representation
    pub const fn as_non_zero_u64(&self) -> NonZeroU64 {
        NonZeroU64::new(u64::from(self.0).saturating_add(1)).expect("Not zero; qed")
    }

    /// Size of blockchain history in pieces
    #[inline(always)]
    pub const fn in_pieces(&self) -> NonZeroU64 {
        NonZeroU64::new(
            u64::from(self.0)
                .saturating_add(1)
                .saturating_mul(RecordedHistorySegment::NUM_PIECES as u64),
        )
        .expect("Not zero; qed")
    }

    /// Segment index that corresponds to this history size
    #[inline(always)]
    pub fn segment_index(&self) -> SegmentIndex {
        self.0
    }

    /// History size at which expiration check for a sector happens.
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
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct ArchivedBlockProgress {
    /// Number of partially archived bytes of a block, `0` for a full block
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

    /// Block is partially archived with the provided number of bytes
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
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct LastArchivedBlock {
    /// Block number
    pub number: Unaligned<BlockNumber>,
    /// Progress of an archived block
    pub archived_progress: ArchivedBlockProgress,
}

impl LastArchivedBlock {
    /// Returns the number of partially archived bytes for a block
    #[inline(always)]
    pub fn partial_archived(&self) -> Option<NonZeroU32> {
        self.archived_progress.partial()
    }

    /// Sets the number of partially archived bytes if block progress was archived partially
    #[inline(always)]
    pub fn set_partial_archived(&mut self, new_partial: NonZeroU32) {
        self.archived_progress = ArchivedBlockProgress::new_partial(new_partial);
    }

    /// Indicate the last archived block was archived fully
    #[inline(always)]
    pub fn set_complete(&mut self) {
        self.archived_progress = ArchivedBlockProgress::new_complete();
    }

    /// Get the block number (unwrap `Unaligned`)
    pub const fn number(&self) -> BlockNumber {
        self.number.as_inner()
    }
}

/// Segment header for a specific segment of a shard
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, TrivialType)]
#[cfg_attr(feature = "scale-codec", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[repr(C)]
pub struct SegmentHeader {
    /// Local segment index
    pub segment_index: Unaligned<LocalSegmentIndex>,
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
        const {
            assert!(size_of::<Self>() <= CHUNK_LEN);
        }
        Blake3Hash::new(
            single_chunk_hash(self.as_bytes())
                .expect("Less than a single chunk worth of bytes; qed"),
        )
    }

    /// Get local segment index (unwrap `Unaligned`)
    #[inline(always)]
    pub const fn local_segment_index(&self) -> LocalSegmentIndex {
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
    /// Number of raw records in one segment of recorded history
    pub const NUM_RAW_RECORDS: usize = 128;
    /// Erasure coding rate for records during the archiving process
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
