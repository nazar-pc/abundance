//! High-performance Merkle Tree and related data structures (Merkle Mountain Range, Sparse Merkle
//! Tree).
//!
//! This crate contains several high-performance Merkle Tree implementations and related data
//! structures, many of which are a subset of each other.
//!
//! These implementations can be an order of magnitude faster than traditional implementations by
//! leveraging the ability of hash functions like BLAKE3 to hash multiple blocks at once with SIMD.
//! Typical Merkle Tree crates only allow customizing a hash function of two leaves, which makes it
//! impossible to benefit from the inherent strength of such hash functions.
//!
//! Currently [`BalancedMerkleTree`], [`UnbalancedMerkleTree`], [`MerkleMountainRange`] and
//! [`SparseMerkleTree`] are available.
//!
//! [`BalancedMerkleTree`] is an optimized special case of [`UnbalancedMerkleTree`], which is in
//! turn an optimized version of [`MerkleMountainRange`], and all 3 will return the same results for
//! identical inputs and hasher.
//!
//! [`SparseMerkleTree`] is a special kind of Merkle Tree, usually of intractable size, where most
//! of the leaves are empty (missing). It will also produce the same root as other trees in case the
//! tree doesn't have any empty leaves, though it is unlikely to happen in practice.
//!
//! [`BalancedMerkleTree`]: balanced::BalancedMerkleTree
//! [`MerkleMountainRange`]: mmr::MerkleMountainRange
//! [`UnbalancedMerkleTree`]: unbalanced::UnbalancedMerkleTree
//! [`SparseMerkleTree`]: sparse::SparseMerkleTree
//!
//! All trees are generic over [`MerkleHasher`], which can hash many pairs of nodes at once and
//! defines the size of nodes. Hashers are created with [`Default`] when they don't depend on the
//! number of leaves, otherwise with [`NewMerkleHasherForTreeSize`], which is implemented for all
//! hashers that implement [`Default`]. Trees that don't know the number of leaves upfront, like
//! [`MerkleMountainRange`], only support hashers that implement [`Default`].
//!
//! `blake3` module contains the BLAKE3 hasher and aliases of all trees that use it (requires
//! `blake3` feature).
//!
//! Does not require a standard library (`no_std`), most APIs are usable without an allocator, never
//! panics.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    const_block_items,
    const_convert,
    const_default,
    const_trait_impl,
    generic_const_args,
    generic_const_items,
    iter_advance_by,
    macroless_generic_const_args,
    maybe_uninit_uninit_array_transpose,
    min_generic_const_args,
    phantom_variance_markers,
    trusted_len
)]
#![no_std]

pub mod balanced;
#[cfg(feature = "blake3")]
pub mod blake3;
pub mod mmr;
pub mod sparse;
pub mod unbalanced;

#[cfg(feature = "alloc")]
extern crate alloc;

use core::mem;

/// Hash function for inner nodes of Merkle Trees.
///
/// Hashing of many pairs at once is a part of the API, such that hash functions like BLAKE3 can use
/// SIMD to hash multiple pairs in parallel.
pub trait MerkleHasher {
    /// Size of a node hash in bytes
    const OUT_LEN: usize;

    /// Hash a pair of nodes together
    fn hash_pair(&self, pair: &[[u8; Self::OUT_LEN]; 2]) -> [u8; Self::OUT_LEN];

    /// Similar to [`Self::hash_pair()`], but hashes multiple pairs at once.
    ///
    /// The default implementation hashes pairs one by one, hash functions that can do better
    /// should override it. An override must return the same as [`Self::hash_pair()`] for each pair,
    /// because trees mix both.
    #[inline(always)]
    fn hash_pairs<const NUM_PAIRS: usize>(
        &self,
        pairs: &[[[u8; Self::OUT_LEN]; 2]; NUM_PAIRS],
    ) -> [[u8; Self::OUT_LEN]; NUM_PAIRS] {
        pairs.each_ref().map(|pair| self.hash_pair(pair))
    }
}

/// Creates hashers for trees with a known number of leaves.
///
/// A hasher either ignores the tree size, in which case it implements [`Default`] and gets this
/// trait through the blanket implementation, or depends on it, in which case it implements this
/// trait directly and must not implement [`Default`]. It can't be both: the two implementations
/// would conflict.
pub const trait NewMerkleHasherForTreeSize: Sized {
    /// Create a hasher for a tree with `num_leaves` leaves
    fn new_for_tree_size(num_leaves: u64) -> Self;
}

/// Hashers that implement [`Default`] ignore the tree size
const impl<H> NewMerkleHasherForTreeSize for H
where
    H: [const] Default,
{
    #[inline(always)]
    fn new_for_tree_size(_num_leaves: u64) -> Self {
        H::default()
    }
}

// TODO: Combine `NUM_PAIRS` and `NUM_LEAVES` into a single generic parameter once compiler is
//  smart enough to deal with it
/// Similar to [`MerkleHasher::hash_pairs()`], but takes pairs as a flat list of nodes.
///
/// # Panics
/// Panics when `NUM_LEAVES != NUM_PAIRS * 2`.
#[inline(always)]
// TODO: Unlock on RISC-V, it started failing since https://github.com/nazar-pc/abundance/pull/551
//  for unknown reason
#[cfg_attr(
    all(feature = "no-panic", not(target_arch = "riscv64")),
    no_panic::no_panic
)]
fn hash_pairs<const NUM_PAIRS: usize, const NUM_LEAVES: usize, H>(
    hasher: &H,
    leaves: &[[u8; H::OUT_LEN]; NUM_LEAVES],
) -> [[u8; H::OUT_LEN]; NUM_PAIRS]
where
    H: MerkleHasher,
{
    // This protects the invariant of the function
    assert_eq!(NUM_LEAVES, NUM_PAIRS * 2);

    // SAFETY: Same size (checked above) and alignment
    let pairs = unsafe {
        mem::transmute::<&[[u8; H::OUT_LEN]; NUM_LEAVES], &[[[u8; H::OUT_LEN]; 2]; NUM_PAIRS]>(
            leaves,
        )
    };
    hasher.hash_pairs(pairs)
}
