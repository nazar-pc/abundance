//! Block-related primitives

pub mod body;
pub mod header;
#[cfg(feature = "alloc")]
pub mod owned;

use crate::block::body::{BeaconChainBody, GenericBlockBody, IntermediateShardBody, LeafShardBody};
use crate::block::header::{
    BeaconChainHeader, GenericBlockHeader, IntermediateShardHeader, LeafShardHeader,
};
#[cfg(feature = "alloc")]
use crate::block::owned::{
    GenericOwnedBlock, OwnedBeaconChainBlock, OwnedBlock, OwnedIntermediateShardBlock,
    OwnedLeafShardBlock,
};
use crate::hashes::Blake3Hash;
use crate::shard::ShardKind;
use crate::solutions::SolutionRange;
#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};
use ab_io_type::trivial_type::TrivialType;
use core::iter::Step;
use core::{fmt, mem};
use derive_more::{
    Add, AddAssign, AsMut, AsRef, Deref, DerefMut, Display, From, Into, Sub, SubAssign,
};
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;

/// Block number
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
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(C)]
pub struct BlockNumber(u64);

impl Step for BlockNumber {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u64::steps_between(&start.0, &end.0)
    }

    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u64::forward_checked(start.0, count).map(Self)
    }

    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u64::backward_checked(start.0, count).map(Self)
    }
}

impl BlockNumber {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u64>();
    /// Genesis block number
    pub const ZERO: BlockNumber = BlockNumber(0);
    /// First block number
    pub const ONE: BlockNumber = BlockNumber(1);
    /// Max block number
    pub const MAX: BlockNumber = BlockNumber(u64::MAX);

    /// Create new instance
    #[inline(always)]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create block number from bytes
    #[inline(always)]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert block number to bytes
    #[inline(always)]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }

    /// Checked addition, returns `None` on overflow
    #[inline(always)]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        if let Some(n) = self.0.checked_add(rhs.0) {
            Some(Self(n))
        } else {
            None
        }
    }

    /// Saturating addition
    #[inline(always)]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    /// Checked subtraction, returns `None` on underflow
    #[inline(always)]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        if let Some(n) = self.0.checked_sub(rhs.0) {
            Some(Self(n))
        } else {
            None
        }
    }

    /// Saturating subtraction
    #[inline(always)]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Block root.
///
/// This is typically called block hash in other blockchains, but here it represents Merkle Tree
/// root of the header rather than a single hash of its contents.
#[derive(
    Debug,
    Display,
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(C)]
pub struct BlockRoot(Blake3Hash);

impl AsRef<[u8]> for BlockRoot {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsMut<[u8]> for BlockRoot {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

impl BlockRoot {
    /// Size in bytes
    pub const SIZE: usize = Blake3Hash::SIZE;

    /// Create new instance
    #[inline(always)]
    pub const fn new(hash: Blake3Hash) -> Self {
        Self(hash)
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn slice_from_repr(value: &[[u8; Self::SIZE]]) -> &[Self] {
        let value = Blake3Hash::slice_from_repr(value);
        // SAFETY: `BlockHash` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion to slice of underlying representation for efficiency purposes
    #[inline(always)]
    pub const fn repr_from_slice(value: &[Self]) -> &[[u8; Self::SIZE]] {
        // SAFETY: `BlockHash` is `#[repr(C)]` and guaranteed to have the same memory layout
        let value = unsafe { mem::transmute::<&[Self], &[Blake3Hash]>(value) };
        Blake3Hash::repr_from_slice(value)
    }
}

/// Generic block
pub trait GenericBlock<'a>
where
    Self: Copy + fmt::Debug,
{
    /// Block header type
    type Header: GenericBlockHeader<'a>;
    /// Block body type
    type Body: GenericBlockBody<'a>;
    /// Owned block
    #[cfg(feature = "alloc")]
    type Owned: GenericOwnedBlock<Block<'a> = Self>
    where
        Self: 'a;

    /// Get block header
    fn header(&self) -> Self::Header;

    /// Get block body
    fn body(&self) -> Self::Body;

    /// Turn into owned version
    #[cfg(feature = "alloc")]
    fn to_owned(self) -> Self::Owned;
}

/// Block that corresponds to the beacon chain
#[derive(Debug, Copy, Clone)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct BeaconChainBlock<'a> {
    /// Block header
    pub header: BeaconChainHeader<'a>,
    /// Block body
    pub body: BeaconChainBody<'a>,
}

impl<'a> BeaconChainBlock<'a> {
    /// Try to create a new instance from provided bytes for provided shard index.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Checks internal consistency of header, body, and block, but no consensus verification is
    /// done. For unchecked version use [`Self::try_from_bytes_unchecked()`].
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = BeaconChainHeader::try_from_bytes(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = BeaconChainBody::try_from_bytes(remainder)?;

        let block = Self { header, body };

        // Check internal consistency
        if !block.is_internally_consistent() {
            return None;
        }

        Some((block, remainder))
    }

    /// Check block's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since full internal consistency is
    /// checked by [`Self::try_from_bytes()`] internally.
    ///
    /// NOTE: This only checks block-level internal consistency, header and block level internal
    /// consistency is checked separately.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        self.body.root() == self.header.result.body_root
            && self.header.child_shard_blocks.len() == self.body.intermediate_shard_blocks.len()
            && self
                .header
                .child_shard_blocks
                .iter()
                .zip(self.body.intermediate_shard_blocks.iter())
                .all(|(child_shard_block_root, intermediate_shard_block)| {
                    child_shard_block_root == &intermediate_shard_block.header.root()
                        && intermediate_shard_block
                            .header
                            .prefix
                            .shard_index
                            .is_child_of(self.header.prefix.shard_index)
                })
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = BeaconChainHeader::try_from_bytes_unchecked(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = BeaconChainBody::try_from_bytes_unchecked(remainder)?;

        Some((Self { header, body }, remainder))
    }

    /// Create an owned version of this block
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBeaconChainBlock {
        OwnedBeaconChainBlock {
            header: self.header.to_owned(),
            body: self.body.to_owned(),
        }
    }
}

impl<'a> GenericBlock<'a> for BeaconChainBlock<'a> {
    type Header = BeaconChainHeader<'a>;
    type Body = BeaconChainBody<'a>;
    #[cfg(feature = "alloc")]
    type Owned = OwnedBeaconChainBlock;

    #[inline(always)]
    fn header(&self) -> Self::Header {
        self.header
    }

    #[inline(always)]
    fn body(&self) -> Self::Body {
        self.body
    }

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }
}

/// Block that corresponds to an intermediate shard
#[derive(Debug, Copy, Clone)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct IntermediateShardBlock<'a> {
    /// Block header
    pub header: IntermediateShardHeader<'a>,
    /// Block body
    pub body: IntermediateShardBody<'a>,
}

impl<'a> IntermediateShardBlock<'a> {
    /// Try to create a new instance from provided bytes for provided shard index.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Checks internal consistency of header, body, and block, but no consensus verification is
    /// done. For unchecked version use [`Self::try_from_bytes_unchecked()`].
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = IntermediateShardHeader::try_from_bytes(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = IntermediateShardBody::try_from_bytes(remainder)?;

        let block = Self { header, body };

        // Check internal consistency
        if !block.is_internally_consistent() {
            return None;
        }

        Some((block, remainder))
    }

    /// Check block's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since full internal consistency is
    /// checked by [`Self::try_from_bytes()`] internally.
    ///
    /// NOTE: This only checks block-level internal consistency, header and block level internal
    /// consistency is checked separately.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        self.body.root() == self.header.result.body_root
            && self.header.child_shard_blocks.len() == self.body.leaf_shard_blocks.len()
            && self
                .header
                .child_shard_blocks
                .iter()
                .zip(self.body.leaf_shard_blocks.iter())
                .all(|(child_shard_block_root, leaf_shard_block)| {
                    child_shard_block_root == &leaf_shard_block.header.root()
                        && leaf_shard_block
                            .header
                            .prefix
                            .shard_index
                            .is_child_of(self.header.prefix.shard_index)
                })
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = IntermediateShardHeader::try_from_bytes_unchecked(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = IntermediateShardBody::try_from_bytes_unchecked(remainder)?;

        Some((Self { header, body }, remainder))
    }

    /// Create an owned version of this block
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedIntermediateShardBlock {
        OwnedIntermediateShardBlock {
            header: self.header.to_owned(),
            body: self.body.to_owned(),
        }
    }
}

impl<'a> GenericBlock<'a> for IntermediateShardBlock<'a> {
    type Header = IntermediateShardHeader<'a>;
    type Body = IntermediateShardBody<'a>;
    #[cfg(feature = "alloc")]
    type Owned = OwnedIntermediateShardBlock;

    #[inline(always)]
    fn header(&self) -> Self::Header {
        self.header
    }

    #[inline(always)]
    fn body(&self) -> Self::Body {
        self.body
    }

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }
}

/// Block that corresponds to a leaf shard
#[derive(Debug, Copy, Clone)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct LeafShardBlock<'a> {
    /// Block header
    pub header: LeafShardHeader<'a>,
    /// Block body
    pub body: LeafShardBody<'a>,
}

impl<'a> LeafShardBlock<'a> {
    /// Try to create a new instance from provided bytes for provided shard index.
    ///
    /// `bytes` should be 8-bytes aligned.
    ///
    /// Checks internal consistency of header, body, and block, but no consensus verification is
    /// done. For unchecked version use [`Self::try_from_bytes_unchecked()`].
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = LeafShardHeader::try_from_bytes(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = LeafShardBody::try_from_bytes(remainder)?;

        let block = Self { header, body };

        // Check internal consistency
        if !block.is_internally_consistent() {
            return None;
        }

        Some((block, remainder))
    }

    /// Check block's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since full internal consistency is
    /// checked by [`Self::try_from_bytes()`] internally.
    ///
    /// NOTE: This only checks block-level internal consistency, header and block level internal
    /// consistency is checked separately.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        self.body.root() == self.header.result.body_root
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        let (header, remainder) = LeafShardHeader::try_from_bytes_unchecked(bytes)?;
        let remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
        let (body, remainder) = LeafShardBody::try_from_bytes_unchecked(remainder)?;

        Some((Self { header, body }, remainder))
    }

    /// Create an owned version of this block
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedLeafShardBlock {
        OwnedLeafShardBlock {
            header: self.header.to_owned(),
            body: self.body.to_owned(),
        }
    }
}

impl<'a> GenericBlock<'a> for LeafShardBlock<'a> {
    type Header = LeafShardHeader<'a>;
    type Body = LeafShardBody<'a>;
    #[cfg(feature = "alloc")]
    type Owned = OwnedLeafShardBlock;

    #[inline(always)]
    fn header(&self) -> Self::Header {
        self.header
    }

    #[inline(always)]
    fn body(&self) -> Self::Body {
        self.body
    }

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }
}

/// Block that contains [`BlockHeader`] and [`BlockBody`]
///
/// [`BlockHeader`]: crate::block::header::BlockHeader
/// [`BlockBody`]: crate::block::body::BlockBody
#[derive(Debug, Copy, Clone, From)]
pub enum Block<'a> {
    /// Block corresponds to the beacon chain
    BeaconChain(BeaconChainBlock<'a>),
    /// Block corresponds to an intermediate shard
    IntermediateShard(IntermediateShardBlock<'a>),
    /// Block corresponds to a leaf shard
    LeafShard(LeafShardBlock<'a>),
}

impl<'a> Block<'a> {
    /// Try to create a new instance from provided bytes.
    ///
    /// `bytes` should be 16-byte aligned.
    ///
    /// Checks internal consistency of header, body, and block, but no consensus verification is
    /// done. For unchecked version use [`Self::try_from_bytes_unchecked()`].
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8], shard_kind: ShardKind) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            ShardKind::BeaconChain => {
                let (block_header, remainder) = BeaconChainBlock::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(block_header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (block_header, remainder) = IntermediateShardBlock::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(block_header), remainder))
            }
            ShardKind::LeafShard => {
                let (block_header, remainder) = LeafShardBlock::try_from_bytes(bytes)?;
                Some((Self::LeafShard(block_header), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    /// Check block's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since full internal consistency is
    /// checked by [`Self::try_from_bytes()`] internally.
    ///
    /// NOTE: This only checks block-level internal consistency, header and block level internal
    /// consistency is checked separately.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        match self {
            Self::BeaconChain(block) => block.is_internally_consistent(),
            Self::IntermediateShard(block) => block.is_internally_consistent(),
            Self::LeafShard(block) => block.is_internally_consistent(),
        }
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(
        bytes: &'a [u8],
        shard_kind: ShardKind,
    ) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            ShardKind::BeaconChain => {
                let (block_header, remainder) = BeaconChainBlock::try_from_bytes_unchecked(bytes)?;
                Some((Self::BeaconChain(block_header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (block_header, remainder) =
                    IntermediateShardBlock::try_from_bytes_unchecked(bytes)?;
                Some((Self::IntermediateShard(block_header), remainder))
            }
            ShardKind::LeafShard => {
                let (block_header, remainder) = LeafShardBlock::try_from_bytes_unchecked(bytes)?;
                Some((Self::LeafShard(block_header), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    /// Create an owned version of this block
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBlock {
        match self {
            Self::BeaconChain(block) => block.to_owned().into(),
            Self::IntermediateShard(block) => block.to_owned().into(),
            Self::LeafShard(block) => block.to_owned().into(),
        }
    }
}

/// BlockWeight type for fork choice rule.
///
/// The smaller the solution range is, the heavier is the block.
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
#[repr(C)]
pub struct BlockWeight(u128);

impl BlockWeight {
    /// Size in bytes
    pub const SIZE: usize = size_of::<u128>();
    /// Zero block weight
    pub const ZERO: BlockWeight = BlockWeight(0);
    /// Max block wright
    pub const MAX: BlockWeight = BlockWeight(u128::MAX);

    /// Create new instance
    #[inline(always)]
    pub const fn new(n: u128) -> Self {
        Self(n)
    }

    /// Derive block weight from provided solution range
    pub const fn from_solution_range(solution_range: SolutionRange) -> Self {
        Self::new((SolutionRange::MAX.as_u64() - solution_range.as_u64()) as u128)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

/// Aligns bytes to `T` and ensures that all padding bytes (if any) are zero
fn align_to_and_ensure_zero_padding<T>(bytes: &[u8]) -> Option<&[u8]> {
    // SAFETY: We do not read `T`, so the contents don't really matter
    let padding = unsafe { bytes.align_to::<T>() }.0;

    // Padding must be zero
    if padding.iter().any(|&byte| byte != 0) {
        return None;
    }

    Some(&bytes[padding.len()..])
}
