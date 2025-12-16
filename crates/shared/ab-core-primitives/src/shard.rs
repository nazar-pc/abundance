//! Shard-related primitives

use crate::nano_u256::NanoU256;
use crate::solutions::{ShardCommitmentHash, ShardMembershipEntropy, SolutionShardCommitment};
use ab_blake3::single_block_keyed_hash;
use ab_io_type::trivial_type::TrivialType;
use core::num::{NonZeroU32, NonZeroU128};
use core::ops::RangeInclusive;
use derive_more::Display;
#[cfg(feature = "scale-codec")]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
#[cfg(feature = "scale-codec")]
use scale_info::TypeInfo;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

const INTERMEDIATE_SHARDS_RANGE: RangeInclusive<u32> = 1..=1023;
const INTERMEDIATE_SHARD_BITS: u32 = 10;
const INTERMEDIATE_SHARD_MASK: u32 = u32::MAX >> (u32::BITS - INTERMEDIATE_SHARD_BITS);

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
        self.0 >= *INTERMEDIATE_SHARDS_RANGE.start() && self.0 <= *INTERMEDIATE_SHARDS_RANGE.end()
    }

    /// Whether the shard index corresponds to an intermediate shard
    #[inline(always)]
    pub const fn is_leaf_shard(&self) -> bool {
        if self.0 <= *INTERMEDIATE_SHARDS_RANGE.end() || self.0 > Self::MAX_SHARD_INDEX {
            return false;
        }

        self.0 & INTERMEDIATE_SHARD_MASK != 0
    }

    /// Whether the shard index corresponds to a real shard
    #[inline(always)]
    pub const fn is_real(&self) -> bool {
        !self.is_phantom_shard()
    }

    /// Whether the shard index corresponds to a phantom shard
    #[inline(always)]
    pub const fn is_phantom_shard(&self) -> bool {
        if self.0 <= *INTERMEDIATE_SHARDS_RANGE.end() || self.0 > Self::MAX_SHARD_INDEX {
            return false;
        }

        self.0 & INTERMEDIATE_SHARD_MASK == 0
    }

    /// Check if this shard is a child shard of `parent`
    #[inline(always)]
    pub const fn is_child_of(self, parent: Self) -> bool {
        match self.shard_kind() {
            Some(ShardKind::BeaconChain) => false,
            Some(ShardKind::IntermediateShard | ShardKind::Phantom) => parent.is_beacon_chain(),
            Some(ShardKind::LeafShard) => {
                // Check that the least significant bits match
                self.0 & INTERMEDIATE_SHARD_MASK == parent.0
            }
            None => false,
        }
    }

    /// Get index of the parent shard (for leaf and intermediate shards)
    #[inline(always)]
    pub const fn parent_shard(self) -> Option<ShardIndex> {
        match self.shard_kind()? {
            ShardKind::BeaconChain => None,
            ShardKind::IntermediateShard | ShardKind::Phantom => Some(ShardIndex::BEACON_CHAIN),
            ShardKind::LeafShard => Some(Self(self.0 & INTERMEDIATE_SHARD_MASK)),
        }
    }

    /// Get shard kind
    #[inline(always)]
    pub const fn shard_kind(&self) -> Option<ShardKind> {
        if self.0 == Self::BEACON_CHAIN.0 {
            Some(ShardKind::BeaconChain)
        } else if self.0 >= *INTERMEDIATE_SHARDS_RANGE.start()
            && self.0 <= *INTERMEDIATE_SHARDS_RANGE.end()
        {
            Some(ShardKind::IntermediateShard)
        } else if self.0 > Self::MAX_SHARD_INDEX {
            None
        } else if self.0 & INTERMEDIATE_SHARD_MASK == 0 {
            // Check if the least significant bits correspond to the beacon chain
            Some(ShardKind::Phantom)
        } else {
            Some(ShardKind::LeafShard)
        }
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
    intermediate_shards: u16,
    /// The number of leaf shards per intermediate shard
    leaf_shards_per_intermediate_shard: u16,
}

impl NumShards {
    /// Create a new instance from a number of intermediate shards and leaf shards per
    /// intermediate shard.
    ///
    /// Returns `None` if inputs are invalid.
    ///
    /// This is typically only necessary for low-level code.
    #[inline(always)]
    pub const fn new(
        intermediate_shards: u16,
        leaf_shards_per_intermediate_shard: u16,
    ) -> Option<Self> {
        if intermediate_shards
            > (*INTERMEDIATE_SHARDS_RANGE.end() - *INTERMEDIATE_SHARDS_RANGE.start() + 1) as u16
        {
            return None;
        }

        let num_shards = Self {
            intermediate_shards,
            leaf_shards_per_intermediate_shard,
        };

        if num_shards.num_leaf_shards() > ShardIndex::MAX_SHARDS.get() {
            return None;
        }

        Some(num_shards)
    }

    /// The number of intermediate shards
    #[inline(always)]
    pub const fn intermediate_shards(self) -> u16 {
        self.intermediate_shards
    }
    /// The number of leaf shards per intermediate shard
    #[inline(always)]
    pub const fn leaf_shards_per_intermediate_shard(self) -> u16 {
        self.leaf_shards_per_intermediate_shard
    }

    /// Total number of leaf shards in the network
    #[inline(always)]
    pub const fn num_leaf_shards(&self) -> u32 {
        self.intermediate_shards as u32 * self.leaf_shards_per_intermediate_shard as u32
    }

    /// Iterator over all intermediate shards
    #[inline(always)]
    pub fn iter_intermediate_shards(&self) -> impl Iterator<Item = ShardIndex> {
        INTERMEDIATE_SHARDS_RANGE
            .take(usize::from(self.intermediate_shards))
            .map(ShardIndex)
    }

    /// Iterator over all intermediate shards
    #[inline(always)]
    pub fn iter_leaf_shards(&self) -> impl Iterator<Item = ShardIndex> {
        self.iter_intermediate_shards()
            .flat_map(|intermediate_shard| {
                (0..self.leaf_shards_per_intermediate_shard as u32).map(move |leaf_shard_index| {
                    ShardIndex((leaf_shard_index << INTERMEDIATE_SHARD_BITS) | intermediate_shard.0)
                })
            })
    }

    /// Derive shard index that should be used in a solution
    #[inline]
    pub fn derive_shard_index(
        &self,
        shard_commitments_root: &ShardCommitmentHash,
        shard_membership_entropy: ShardMembershipEntropy,
    ) -> ShardIndex {
        // The complexity of this whole function is primarily caused by the fact that the invariant
        // of the total number of shards can't be fully enforced here
        let total_shards_to_consider = self.num_leaf_shards().clamp(
            self.intermediate_shards.max(1) as u32,
            ShardIndex::MAX_SHARDS.get(),
        );

        let hash =
            single_block_keyed_hash(shard_commitments_root, shard_membership_entropy.as_bytes())
                .expect("Input is smaller than block size; qed");
        // Going through `NanoU256` because the total number of shards is not guaranteed to be a
        // power of two
        let shard_index_offset =
            NanoU256::from_le_bytes(hash) % u64::from(total_shards_to_consider);

        if self.num_leaf_shards() == 0 {
            self.iter_intermediate_shards()
                .nth(shard_index_offset as usize)
                .unwrap_or(ShardIndex::BEACON_CHAIN)
        } else {
            self.iter_leaf_shards()
                .nth(shard_index_offset as usize)
                .unwrap_or(ShardIndex::BEACON_CHAIN)
        }
    }

    /// Derive shard commitment index that should be used in a solution.
    ///
    /// Returned index is always within `0`..[`SolutionShardCommitment::NUM_LEAVES`] range.
    #[inline]
    pub fn derive_shard_commitment_index(
        &self,
        shard_commitments_root: &ShardCommitmentHash,
        shard_membership_entropy: &ShardMembershipEntropy,
    ) -> u32 {
        let hash =
            single_block_keyed_hash(shard_commitments_root, shard_membership_entropy.as_bytes())
                .expect("Input is smaller than block size; qed");
        const {
            assert!(SolutionShardCommitment::NUM_LEAVES.is_power_of_two());
        }
        u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]])
            % SolutionShardCommitment::NUM_LEAVES as u32
    }
}
