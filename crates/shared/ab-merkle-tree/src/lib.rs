//! Merkle Tree and related data structures.
//!
//! This crate contains several Merkle Tree implementations and related data structures, many of
//! which are a subset of each other.
//!
//! Currently [`BalancedMerkleTree`], [`UnbalancedMerkleTree`] and [`MerkleMountainRange`] are
//! available. [`BalancedMerkleTree`] is an optimized special case of [`UnbalancedMerkleTree`],
//! which is in turn an optimized version of [`MerkleMountainRange`] and all 3 will return the same
//! results for identical inputs.
//!
//! [`BalancedMerkleTree`]: balanced::BalancedMerkleTree
//! [`MerkleMountainRange`]: mmr::MerkleMountainRange
//! [`UnbalancedMerkleTree`]: unbalanced::UnbalancedMerkleTree

#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    generic_const_exprs,
    iter_advance_by,
    maybe_uninit_slice,
    maybe_uninit_uninit_array_transpose,
    ptr_as_ref_unchecked,
    trusted_len
)]
#![no_std]

// TODO: Consider domains-specific internal node separator and inclusion of tree size into hashing
//  key
pub mod balanced;
pub mod mmr;
pub mod unbalanced;

#[cfg(feature = "alloc")]
extern crate alloc;

use ab_blake3::{BLOCK_LEN, KEY_LEN, OUT_LEN};

/// Used as a key in keyed blake3 hash for inner nodes of Merkle Trees.
///
/// This value is a blake3 hash of as string `merkle-tree-inner-node`.
pub const INNER_NODE_DOMAIN_SEPARATOR: [u8; KEY_LEN] =
    ab_blake3::const_hash(b"merkle-tree-inner-node");

/// Helper function to hash two nodes together using [`ab_blake3::single_block_keyed_hash()`] and
/// [`INNER_NODE_DOMAIN_SEPARATOR`]
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn hash_pair(left: &[u8; OUT_LEN], right: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
    let mut pair = [0u8; OUT_LEN * 2];
    pair[..OUT_LEN].copy_from_slice(left);
    pair[OUT_LEN..].copy_from_slice(right);

    ab_blake3::single_block_keyed_hash(&INNER_NODE_DOMAIN_SEPARATOR, &pair)
        .expect("Exactly one block worth of data; qed")
}

/// Similar to [`hash_pair()`] but already has left and right nodes concatenated
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn hash_pair_block(pair: &[u8; BLOCK_LEN]) -> [u8; OUT_LEN] {
    ab_blake3::single_block_keyed_hash(&INNER_NODE_DOMAIN_SEPARATOR, pair)
        .expect("Exactly one block worth of data; qed")
}

/// Similar to [`hash_pair_block()`] but supports processing multiple blocks at once
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn hash_pair_blocks<const NUM_BLOCKS: usize>(
    pairs: &[[u8; BLOCK_LEN]; NUM_BLOCKS],
) -> [[u8; OUT_LEN]; NUM_BLOCKS] {
    let mut hashes = [[0; OUT_LEN]; NUM_BLOCKS];
    ab_blake3::single_block_keyed_hash_many_exact(&INNER_NODE_DOMAIN_SEPARATOR, pairs, &mut hashes);
    hashes
}
