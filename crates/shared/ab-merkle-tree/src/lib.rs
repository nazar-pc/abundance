//! Merkle Tree implementations.
//!
//! This crate contains several Merkle Tree implementations that are a subset of each other.
//!
//! Currently [`BalancedHashedMerkleTree`] and [`UnbalancedHashedMerkleTree`] are available, with
//! [`BalancedHashedMerkleTree`] being an optimized special case of [`UnbalancedHashedMerkleTree`]
//! and both return the same results for identical inputs.
//!
//! [`BalancedHashedMerkleTree`]: balanced_hashed::BalancedHashedMerkleTree
//! [`UnbalancedHashedMerkleTree`]: unbalanced_hashed::UnbalancedHashedMerkleTree

#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    array_chunks,
    generic_arg_infer,
    generic_const_exprs,
    maybe_uninit_slice,
    maybe_uninit_uninit_array_transpose,
    ptr_as_ref_unchecked,
    trusted_len
)]
#![no_std]

// TODO: Consider domains-specific internal node separator and inclusion of tree size into hashing
//  key
pub mod balanced_hashed;
pub mod unbalanced_hashed;

#[cfg(feature = "alloc")]
extern crate alloc;

use blake3::{KEY_LEN, OUT_LEN};

/// Used as a key in keyed blake3 hash for inner nodes of Merkle Trees.
///
/// This value is a blake3 hash of as string `merkle-tree-inner-node`.
// TODO: Replace with hashing once https://github.com/BLAKE3-team/BLAKE3/issues/440 is resolved
pub const INNER_NODE_DOMAIN_SEPARATOR: [u8; KEY_LEN] = [
    0x53, 0x11, 0x7d, 0x4d, 0xa8, 0x1a, 0x34, 0x35, 0x0b, 0x1a, 0x30, 0xd4, 0x28, 0x6d, 0x7e, 0x5a,
    0x1e, 0xb0, 0xa2, 0x0f, 0x5e, 0x5e, 0x26, 0x94, 0x47, 0x4b, 0x4f, 0xbd, 0x86, 0xc3, 0xc0, 0x7e,
];

/// Helper function to hash two nodes together using [`blake3::keyed_hash()`] and
/// [`INNER_NODE_DOMAIN_SEPARATOR`]
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
pub fn hash_pair(left: &[u8; OUT_LEN], right: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
    let mut pair = [0u8; OUT_LEN * 2];
    pair[..OUT_LEN].copy_from_slice(left);
    pair[OUT_LEN..].copy_from_slice(right);

    blake3::keyed_hash(&INNER_NODE_DOMAIN_SEPARATOR, &pair).into()
}
