//! Fast hashers that tests use instead of BLAKE3

use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize, balanced, mmr, sparse, unbalanced};
use array_reshape::Flatten;

/// Size of a node hash of test hashers in bytes, different from BLAKE3 to exercise generic sizes
pub(crate) const OUT_LEN: usize = 16;

pub(crate) type BalancedMerkleTree<'a, const N: usize> =
    balanced::BalancedMerkleTree<'a, N, TestHasher>;
pub(crate) type MerkleMountainRange<const MAX_N: u64> = mmr::MerkleMountainRange<MAX_N, TestHasher>;
pub(crate) type SparseMerkleTree<const BITS: u8> = sparse::SparseMerkleTree<BITS, TestHasher>;
pub(crate) type UnbalancedMerkleTree = unbalanced::UnbalancedMerkleTree<TestHasher>;

/// Hasher that doesn't depend on the tree size
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[derive_const(Default)]
pub(crate) struct TestHasher;

impl MerkleHasher for TestHasher {
    const OUT_LEN: usize = OUT_LEN;

    fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
        hash_pair_with_seed(0, pair)
    }
}

/// Hasher that doesn't depend on the tree size, but hashes a single leaf of a tree
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[derive_const(Default)]
pub(crate) struct SingleLeafTestHasher;

impl MerkleHasher for SingleLeafTestHasher {
    const OUT_LEN: usize = OUT_LEN;

    fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
        hash_pair_with_seed(0, pair)
    }

    fn hash_single_leaf(&self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
        hash_pair_with_seed(1, &[*leaf, [0; OUT_LEN]])
    }
}

/// Hasher that depends on the tree size, it only implements [`NewMerkleHasherForTreeSize`]
#[derive(Debug, Copy, Clone)]
pub(crate) struct TreeSizeTestHasher {
    num_leaves: u64,
}

const impl NewMerkleHasherForTreeSize for TreeSizeTestHasher {
    fn new_for_tree_size(num_leaves: u64) -> Self {
        Self { num_leaves }
    }
}

impl MerkleHasher for TreeSizeTestHasher {
    const OUT_LEN: usize = OUT_LEN;

    fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
        hash_pair_with_seed(self.num_leaves, pair)
    }

    fn hash_single_leaf(&self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
        hash_pair_with_seed(self.num_leaves, &[*leaf, [0; OUT_LEN]])
    }
}

/// Hash a pair of nodes with [`TestHasher`]
pub(crate) fn hash_pair(left: &[u8; OUT_LEN], right: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
    TestHasher.hash_pair(&[*left, *right])
}

/// Finalizer of SplitMix64
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Mixes every 64-bit word of the input into two lanes, such that any change of the input or its
/// order changes the whole output
fn hash_pair_with_seed(seed: u64, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
    let mut low = mix(seed ^ 0x9e37_79b9_7f4a_7c15);
    let mut high = mix(low);

    for word in pair.as_flattened().as_chunks::<8>().0 {
        low = mix(low ^ u64::from_le_bytes(*word));
        high = mix(high.wrapping_add(low));
    }

    [low.to_le_bytes(), high.to_le_bytes()].flatten()
}
