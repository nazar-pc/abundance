//! BLAKE3 hasher and aliases of Merkle Trees that use it

use crate::{MerkleHasher, balanced, mmr, sparse, unbalanced};
use ab_blake3::{BLOCK_LEN, KEY_LEN, OUT_LEN};
use array_reshape::{Flatten, FlattenEach};

/// [`BalancedMerkleTree`](balanced::BalancedMerkleTree) with the default BLAKE3 hasher
pub type BalancedMerkleTree<'a, const N: usize> =
    balanced::BalancedMerkleTree<'a, N, Blake3MerkleHasher>;
/// [`MerkleMountainRange`](mmr::MerkleMountainRange) with the default BLAKE3 hasher
pub type MerkleMountainRange<const MAX_N: u64> =
    mmr::MerkleMountainRange<MAX_N, Blake3MerkleHasher>;
/// [`SparseMerkleTree`](sparse::SparseMerkleTree) with the default BLAKE3 hasher
pub type SparseMerkleTree<const BITS: u8> = sparse::SparseMerkleTree<BITS, Blake3MerkleHasher>;
/// [`UnbalancedMerkleTree`](unbalanced::UnbalancedMerkleTree) with the default BLAKE3 hasher
pub type UnbalancedMerkleTree = unbalanced::UnbalancedMerkleTree<Blake3MerkleHasher>;

/// Keyed BLAKE3 hasher of inner nodes with [`Self::KEY`], which hashes multiple pairs at once with
/// SIMD
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Blake3MerkleHasher;

const impl Default for Blake3MerkleHasher {
    #[inline(always)]
    fn default() -> Self {
        Self
    }
}

impl Blake3MerkleHasher {
    /// Key of keyed BLAKE3 hash, BLAKE3 hash of a string `merkle-tree-inner-node`
    pub const KEY: [u8; KEY_LEN] = ab_blake3::const_hash(b"merkle-tree-inner-node");
}

impl MerkleHasher for Blake3MerkleHasher {
    const OUT_LEN: usize = OUT_LEN;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
        ab_blake3::single_block_keyed_hash(&Self::KEY, pair.flatten_ref::<BLOCK_LEN>())
            .expect("Exactly one block worth of data; qed")
    }

    #[inline(always)]
    // TODO: Unlock on RISC-V, it started failing since https://github.com/nazar-pc/abundance/pull/551
    //  for unknown reason
    #[cfg_attr(
        all(feature = "no-panic", not(target_arch = "riscv64")),
        no_panic::no_panic
    )]
    fn hash_pairs<const NUM_PAIRS: usize>(
        &self,
        pairs: &[[[u8; OUT_LEN]; 2]; NUM_PAIRS],
    ) -> [[u8; OUT_LEN]; NUM_PAIRS] {
        let pairs = pairs
            .as_slice()
            .flatten_each_ref::<BLOCK_LEN>()
            .as_array::<NUM_PAIRS>()
            .expect("Same number of pairs as in the input; qed");
        let mut hashes = [[0; OUT_LEN]; NUM_PAIRS];
        ab_blake3::single_block_keyed_hash_many_exact(&Self::KEY, pairs, &mut hashes);
        hashes
    }
}
