//! Shard-related primitives

use ab_io_type::trivial_type::TrivialType;
use core::num::{NonZeroU32, NonZeroU128};
use derive_more::Display;
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A kind of shard.
///
/// Schematically, the hierarchy of shards is as follows:
/// ```text
///                          Beacon chain
///                          /          \
///      Intermediate shard 1            Intermediate shard 2
///              /  \                            /  \
/// Leaf shard 11   Leaf shard 12   Leaf shard 22   Leaf shard 22
/// ```
///
/// Beacon chain has index 0, intermediate shards indices 1..=1023. Leaf shards share the same least
/// significant 10 bits as their respective intermediate shards, so leaf shards of intermediate
/// shard 1 have indices like 1025,2049,3097,etc.
///
/// If represented as least significant bits first (as it will be in the formatted address):
/// ```text
/// Intermediate shard bits
///     \            /
///      1000_0000_0001_0000_0000
///                 /            \
///                Leaf shard bits
/// ```
///
/// Note that shards that have 10 least significant bits set to 0 (corresponds to the beacon chain)
/// are not leaf shards, in fact, they are not even physical shards that have nodes in general. The
/// meaning of these shards is TBD, currently they are called "phantom" shards and may end up
/// containing some system contracts with special meaning, but no blocks will ever exist for such
/// shards.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ShardKind {
    /// Beacon chain shard
    BeaconChain,
    /// Intermediate shard directly below the beacon chain that has child shards
    IntermediateShard,
    /// Leaf shard, which doesn't have child shards
    LeafShard,
    /// TODO
    Phantom,
}

impl ShardKind {
    /// Try to convert to real shard kind.
    ///
    /// Returns `None` for phantom shard.
    #[inline(always)]
    pub fn to_real(self) -> Option<RealShardKind> {
        match self {
            ShardKind::BeaconChain => Some(RealShardKind::BeaconChain),
            ShardKind::IntermediateShard => Some(RealShardKind::IntermediateShard),
            ShardKind::LeafShard => Some(RealShardKind::LeafShard),
            ShardKind::Phantom => None,
        }
    }
}

/// Real shard kind for which a block may exist, see [`ShardKind`] for more details
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum RealShardKind {
    /// Beacon chain shard
    BeaconChain,
    /// Intermediate shard directly below the beacon chain that has child shards
    IntermediateShard,
    /// Leaf shard, which doesn't have child shards
    LeafShard,
}

impl From<RealShardKind> for ShardKind {
    #[inline(always)]
    fn from(shard_kind: RealShardKind) -> Self {
        match shard_kind {
            RealShardKind::BeaconChain => ShardKind::BeaconChain,
            RealShardKind::IntermediateShard => ShardKind::IntermediateShard,
            RealShardKind::LeafShard => ShardKind::LeafShard,
        }
    }
}

/// Shard index
#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct ShardIndex(u32);

impl ShardIndex {
    /// Beacon chain
    pub const BEACON_CHAIN: Self = Self(0);
    /// Max possible shard index
    pub const MAX_SHARD_INDEX: u32 = Self::MAX_SHARDS.get() - 1;
    /// Max possible number of shards
    pub const MAX_SHARDS: NonZeroU32 = NonZeroU32::new(2u32.pow(20)).expect("Not zero; qed");
    /// Max possible number of addresses per shard
    pub const MAX_ADDRESSES_PER_SHARD: NonZeroU128 =
        NonZeroU128::new(2u128.pow(108)).expect("Not zero; qed");

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

    /// Whether the shard index corresponds to the beacon chain
    #[inline(always)]
    pub const fn is_beacon_chain(&self) -> bool {
        self.0 == Self::BEACON_CHAIN.0
    }

    /// Whether the shard index corresponds to an intermediate shard
    #[inline(always)]
    pub const fn is_intermediate_shard(&self) -> bool {
        self.0 >= 1 && self.0 <= 1023
    }

    /// Whether the shard index corresponds to an intermediate shard
    #[inline(always)]
    pub const fn is_leaf_shard(&self) -> bool {
        if self.0 <= 1023 || self.0 > Self::MAX_SHARD_INDEX {
            return false;
        }

        self.0 & 0b11_1111_1111 != 0
    }

    /// Whether the shard index corresponds to a real shard
    #[inline(always)]
    pub const fn is_real(&self) -> bool {
        !self.is_phantom_shard()
    }

    /// Whether the shard index corresponds to a phantom shard
    #[inline(always)]
    pub const fn is_phantom_shard(&self) -> bool {
        if self.0 <= 1023 || self.0 > Self::MAX_SHARD_INDEX {
            return false;
        }

        self.0 & 0b11_1111_1111 == 0
    }

    /// Check if this shard is a child shard of `parent`
    #[inline]
    pub const fn is_child_of(self, parent: Self) -> bool {
        match self.shard_kind() {
            Some(ShardKind::BeaconChain) => false,
            Some(ShardKind::IntermediateShard | ShardKind::Phantom) => parent.is_beacon_chain(),
            Some(ShardKind::LeafShard) => {
                // Check that the least significant bits match
                self.0 & 0b11_1111_1111 == parent.0
            }
            None => false,
        }
    }

    /// Get index of the parent shard (for leaf and intermediate shards)
    #[inline]
    pub const fn parent_shard(self) -> Option<ShardIndex> {
        match self.shard_kind()? {
            ShardKind::BeaconChain => None,
            ShardKind::IntermediateShard | ShardKind::Phantom => Some(ShardIndex::BEACON_CHAIN),
            ShardKind::LeafShard => Some(Self(self.0 & 0b11_1111_1111)),
        }
    }

    /// Get shard kind
    #[inline(always)]
    pub const fn shard_kind(&self) -> Option<ShardKind> {
        Some(match self.0 {
            0 => ShardKind::BeaconChain,
            1..=1023 => ShardKind::IntermediateShard,
            shard_index => {
                if shard_index > Self::MAX_SHARD_INDEX {
                    return None;
                }

                // Check if the least significant bits correspond to the beacon chain
                if shard_index & 0b11_1111_1111 == 0 {
                    ShardKind::Phantom
                } else {
                    ShardKind::LeafShard
                }
            }
        })
    }
}

/// Number of shards in the network
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[cfg_attr(
    feature = "scale-codec",
    derive(Encode, Decode, TypeInfo, MaxEncodedLen)
)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct NumShards {
    /// The number of intermediate shards
    pub intermediate_shards: u16,
    /// The number of leaf shards per intermediate shard
    pub leaf_shards_per_intermediate_shard: u16,
}

impl NumShards {
    /// Total number of leaf shards in the network
    #[inline(always)]
    pub fn num_leaf_shards(&self) -> u32 {
        self.intermediate_shards as u32 * self.leaf_shards_per_intermediate_shard as u32
    }

    // TODO: APIs for enumerating/iterating shards based on the specified fields
}
