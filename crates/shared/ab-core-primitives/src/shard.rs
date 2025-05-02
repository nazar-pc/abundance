//! Shard-related primitives

use ab_io_type::trivial_type::TrivialType;
use core::num::{NonZeroU32, NonZeroU128};
use derive_more::Display;

/// Shard index
#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, TrivialType)]
#[repr(transparent)]
pub struct ShardIndex(u32);

impl ShardIndex {
    /// Max possible shard index
    pub const MAX_SHARD_INDEX: u32 = Self::MAX_SHARDS.get() - 1;
    /// Max possible number of shards
    pub const MAX_SHARDS: NonZeroU32 = NonZeroU32::new(2u32.pow(20)).expect("Not zero; qed");
    /// Max possible number of addresses per shard
    pub const MAX_ADDRESSES_PER_SHARD: NonZeroU128 =
        NonZeroU128::new((u128::MAX / 2 + 1) / (Self::MAX_SHARDS.get() as u128 / 2))
            .expect("Not zero; qed");

    // TODO: Remove once traits work in const environment and `From` could be used
    /// Create shard index from `u32`.
    ///
    /// Returns `None` if `shard_index > ShardIndex::MAX_SHARD_INDEX`
    ///
    /// This is typically only necessary for low-level code.
    #[inline(always)]
    pub const fn new(shard_index: u32) -> Option<Self> {
        if shard_index > Self::MAX_SHARD_INDEX {
            return None;
        }

        Some(Self(shard_index))
    }

    // TODO: Remove once traits work in const environment and `From` could be used
    /// Convert shard index to `u32`.
    ///
    /// This is typically only necessary for low-level code.
    #[inline(always)]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}
