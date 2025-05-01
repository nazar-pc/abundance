//! Block-related primitives

#[cfg(feature = "serde")]
use ::serde::{Deserialize, Serialize};
use core::iter::Step;
use derive_more::{Add, AddAssign, Display, From, Into, Sub, SubAssign};
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
)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct BlockNumber(u64);

impl Step for BlockNumber {
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
    #[inline]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Get internal representation
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Create block number from bytes
    #[inline]
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u64::from_le_bytes(bytes))
    }

    /// Convert block number to bytes
    #[inline]
    pub const fn to_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_le_bytes()
    }

    /// Checked subtraction, returns `None` on underflow
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        if let Some(n) = self.0.checked_sub(rhs.0) {
            Some(Self(n))
        } else {
            None
        }
    }

    /// Saturating subtraction
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

// TODO: New type
/// Block hash in Subspace network
pub type BlockHash = [u8; 32];

// TODO: New type
/// BlockWeight type for fork choice rule.
///
/// The smaller the solution range is, the heavier is the block.
pub type BlockWeight = u128;
