//! BLAKE3 hasher and aliases of Merkle Trees that use it with the default key

use crate::{MerkleHasher, balanced, mmr, sparse, unbalanced};
use ab_blake3::{BLOCK_LEN, KEY_LEN, OUT_LEN};
use array_reshape::{Flatten, FlattenEach, Rechunk};
use core::cmp::Ordering;
use core::fmt;
use core::marker::PhantomCovariant;

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

/// Key of keyed BLAKE3 hash used by [`Blake3MerkleHasher`] for inner nodes
pub trait Blake3MerkleKey {
    /// Key bytes
    const KEY: [u8; KEY_LEN];
}

/// Default key, BLAKE3 hash of a string `merkle-tree-inner-node`
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct DefaultBlake3MerkleKey;

impl Blake3MerkleKey for DefaultBlake3MerkleKey {
    const KEY: [u8; KEY_LEN] = ab_blake3::const_hash(b"merkle-tree-inner-node");
}

/// Keyed BLAKE3 hasher of inner nodes, which hashes multiple pairs at once with SIMD.
///
/// The key is customizable with `K`, the default key is [`DefaultBlake3MerkleKey`].
pub struct Blake3MerkleHasher<K = DefaultBlake3MerkleKey>(PhantomCovariant<K>);

// Traits below are implemented manually rather than derived to avoid bounds on `K`

const impl<K> Default for Blake3MerkleHasher<K> {
    #[inline(always)]
    fn default() -> Self {
        Self(PhantomCovariant::new())
    }
}

impl<K> fmt::Debug for Blake3MerkleHasher<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Blake3MerkleHasher").field(&self.0).finish()
    }
}

impl<K> Clone for Blake3MerkleHasher<K> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for Blake3MerkleHasher<K> {}

impl<K> PartialEq for Blake3MerkleHasher<K> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K> Eq for Blake3MerkleHasher<K> {}

impl<K> PartialOrd for Blake3MerkleHasher<K> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K> Ord for Blake3MerkleHasher<K> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

// `<Self as MerkleHasher>::OUT_LEN` doesn't unify with `OUT_LEN` in generic code, hence reshaping
impl<K> MerkleHasher for Blake3MerkleHasher<K>
where
    K: Blake3MerkleKey,
{
    const OUT_LEN: usize = OUT_LEN;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn hash_pair(
        &self,
        pair: &[[u8; <Self as MerkleHasher>::OUT_LEN]; 2],
    ) -> [u8; <Self as MerkleHasher>::OUT_LEN] {
        let hash = ab_blake3::single_block_keyed_hash(&K::KEY, pair.flatten_ref::<BLOCK_LEN>());
        // Reshaping inside `Option` results in fewer copies of the hash than after `expect()`
        hash.map(|hash| [hash].flatten())
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
        pairs: &[[[u8; <Self as MerkleHasher>::OUT_LEN]; 2]; NUM_PAIRS],
    ) -> [[u8; <Self as MerkleHasher>::OUT_LEN]; NUM_PAIRS] {
        let pairs = pairs
            .as_slice()
            .flatten_each_ref::<BLOCK_LEN>()
            .as_array::<NUM_PAIRS>()
            .expect("Same number of pairs as in the input; qed");
        let mut hashes = [[0; <Self as MerkleHasher>::OUT_LEN]; NUM_PAIRS];
        ab_blake3::single_block_keyed_hash_many_exact(&K::KEY, pairs, hashes.rechunk_mut());
        hashes
    }
}
