use ab_blake3::{BLOCK_LEN, KEY_LEN, MAX_SIMD_DEGREE, OUT_LEN};
use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize};
use array_reshape::{Flatten, FlattenEach, Rechunk};
use const_fn_specialization::const_fn_specialization;
use core::gca;

/// Keyed BLAKE3 hasher of inner nodes with [`Self::KEY`], which doesn't depend on the tree size.
///
/// Used for trees with a number of leaves known at compile time, the MMR and sparse trees.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Blake3StaticMerkleHasher<const KEY: [u8; KEY_LEN]>;

// TODO: Define `OUT_LEN` as `gca!(OUT_LEN)` in both implementations and use `[u8; OUT_LEN]` in
//  signatures, which makes reshaping in their bodies unnecessary, once
//  https://github.com/rust-lang/rust/issues/166409 is resolved
impl<const KEY: [u8; KEY_LEN]> MerkleHasher for Blake3StaticMerkleHasher<KEY> {
    const OUT_LEN: usize = OUT_LEN;
    const PARALLEL_PAIRS: usize = MAX_SIMD_DEGREE;

    #[inline(always)]
    fn hash_pair(&self, pair: &[[u8; gca!(Self::OUT_LEN)]; 2]) -> [u8; gca!(Self::OUT_LEN)] {
        let hash = ab_blake3::single_block_keyed_hash(self.node_key(), pair.as_flattened());
        // Reshaping inside `Option` results in fewer copies of the hash than after `expect()`
        hash.map(|hash| [hash].flatten())
            .expect("Exactly one block worth of data; qed")
    }

    #[inline(always)]
    fn hash_pairs<const NUM_PAIRS: usize>(
        &self,
        pairs: &[[[u8; gca!(Self::OUT_LEN)]; 2]; NUM_PAIRS],
    ) -> [[u8; gca!(Self::OUT_LEN)]; NUM_PAIRS] {
        let mut hashes = [[0; _]; NUM_PAIRS];
        ab_blake3::single_block_keyed_hash_many_exact(
            self.node_key(),
            pairs.flatten_each_ref::<BLOCK_LEN>(),
            hashes.rechunk_mut(),
        );
        hashes
    }

    #[inline(always)]
    fn hash_single_leaf(&self, leaf: &[u8; gca!(Self::OUT_LEN)]) -> [u8; gca!(Self::OUT_LEN)] {
        [self.single_leaf_root(core::array::from_ref(leaf).flatten_ref::<OUT_LEN>())].flatten()
    }
}

const impl<const KEY: [u8; KEY_LEN]> Default for Blake3StaticMerkleHasher<KEY> {
    #[inline(always)]
    fn default() -> Self {
        Self
    }
}

impl<const KEY: [u8; KEY_LEN]> Blake3StaticMerkleHasher<KEY> {
    /// Key of keyed BLAKE3 hash
    pub const KEY: [u8; KEY_LEN] = KEY;

    #[inline(always)]
    fn node_key(&self) -> &[u8; KEY_LEN] {
        &KEY
    }

    /// The key doesn't depend on the tree size, so a single leaf is the root of the tree
    #[inline(always)]
    fn single_leaf_root(self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
        *leaf
    }
}

/// Keyed BLAKE3 hasher of inner nodes with [`Self::key()`], which commits to the tree size.
///
/// Used for trees with a number of leaves that is only known at runtime.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Blake3SizedMerkleHasher<const BASE_KEY: [u8; KEY_LEN]> {
    key: [u8; KEY_LEN],
}

impl<const BASE_KEY: [u8; KEY_LEN]> MerkleHasher for Blake3SizedMerkleHasher<BASE_KEY> {
    const OUT_LEN: usize = OUT_LEN;
    const PARALLEL_PAIRS: usize = MAX_SIMD_DEGREE;

    #[inline(always)]
    fn hash_pair(&self, pair: &[[u8; gca!(Self::OUT_LEN)]; 2]) -> [u8; gca!(Self::OUT_LEN)] {
        let hash = ab_blake3::single_block_keyed_hash(self.node_key(), pair.as_flattened());
        // Reshaping inside `Option` results in fewer copies of the hash than after `expect()`
        hash.map(|hash| [hash].flatten())
            .expect("Exactly one block worth of data; qed")
    }

    #[inline(always)]
    fn hash_pairs<const NUM_PAIRS: usize>(
        &self,
        pairs: &[[[u8; gca!(Self::OUT_LEN)]; 2]; NUM_PAIRS],
    ) -> [[u8; gca!(Self::OUT_LEN)]; NUM_PAIRS] {
        let mut hashes = [[0; _]; NUM_PAIRS];
        ab_blake3::single_block_keyed_hash_many_exact(
            self.node_key(),
            pairs.flatten_each_ref::<BLOCK_LEN>(),
            hashes.rechunk_mut(),
        );
        hashes
    }

    #[inline(always)]
    fn hash_single_leaf(&self, leaf: &[u8; gca!(Self::OUT_LEN)]) -> [u8; gca!(Self::OUT_LEN)] {
        [self.single_leaf_root(core::array::from_ref(leaf).flatten_ref::<OUT_LEN>())].flatten()
    }
}

const impl<const BASE_KEY: [u8; KEY_LEN]> NewMerkleHasherForTreeSize
    for Blake3SizedMerkleHasher<BASE_KEY>
{
    #[inline(always)]
    fn new_for_tree_size(num_leaves: u64) -> Self {
        Self {
            key: Self::key(num_leaves),
        }
    }
}

#[const_fn_specialization]
impl<const BASE_KEY: [u8; KEY_LEN]> Blake3SizedMerkleHasher<BASE_KEY> {
    pub(super) const BASE_KEY: [u8; KEY_LEN] = BASE_KEY;

    /// Key of keyed BLAKE3 hash for a tree with `num_leaves` leaves
    #[inline(always)]
    pub const fn key(num_leaves: u64) -> [u8; KEY_LEN] {
        ab_blake3::const_keyed_hash(&BASE_KEY, &num_leaves.to_le_bytes())
    }

    /// Key of keyed BLAKE3 hash for a tree with `num_leaves` leaves
    #[inline(always)]
    pub fn key(num_leaves: u64) -> [u8; KEY_LEN] {
        ab_blake3::single_block_keyed_hash(&BASE_KEY, &num_leaves.to_le_bytes())
            .expect("Less than a single block worth of bytes; qed")
    }

    #[inline(always)]
    fn node_key(&self) -> &[u8; KEY_LEN] {
        &self.key
    }

    /// The key commits to the tree size, so a single leaf is hashed with it too
    #[inline(always)]
    fn single_leaf_root(&self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
        ab_blake3::single_block_keyed_hash(&self.key, leaf)
            .expect("Less than a single block worth of bytes; qed")
    }
}
