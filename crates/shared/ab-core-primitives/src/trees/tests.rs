use crate::trees::{BlockMerkleMountainRangeHasher, TransactionsMerkleHasher};
use ab_blake3::{KEY_LEN, OUT_LEN};
use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};

fn random_pairs<const NUM_PAIRS: usize>() -> [[[u8; OUT_LEN]; 2]; NUM_PAIRS] {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut pairs = [[[0u8; OUT_LEN]; 2]; NUM_PAIRS];
    for node in pairs.as_flattened_mut() {
        rng.fill_bytes(node);
    }
    pairs
}

fn test_hasher<const NUM_PAIRS: usize, Hasher>(hasher: &Hasher, key: &[u8; KEY_LEN])
where
    Hasher: MerkleHasher<OUT_LEN = { OUT_LEN }>,
{
    let pairs = random_pairs::<NUM_PAIRS>();
    let hashes = pairs.each_ref().map(|pair| hasher.hash_pair(pair));

    for (pair, hash) in pairs.iter().zip(&hashes) {
        assert_eq!(
            *hash,
            ab_blake3::const_keyed_hash(key, pair.as_flattened()),
            "NUM_PAIRS {NUM_PAIRS}"
        );
    }
    assert_eq!(hasher.hash_pairs(&pairs), hashes, "NUM_PAIRS {NUM_PAIRS}");
}

#[test]
fn static_hasher() {
    let hasher = BlockMerkleMountainRangeHasher::default();
    let key = &BlockMerkleMountainRangeHasher::KEY;
    test_hasher::<1, _>(&hasher, key);
    test_hasher::<3, _>(&hasher, key);
    test_hasher::<16, _>(&hasher, key);
    test_hasher::<17, _>(&hasher, key);

    // Doesn't depend on the tree size, so a single leaf is the root as is
    let [[leaf, _]] = random_pairs::<1>();
    assert_eq!(
        BlockMerkleMountainRangeHasher::default().hash_single_leaf(&leaf),
        leaf
    );
}

#[test]
fn sized_hasher() {
    const KEY: [u8; KEY_LEN] = TransactionsMerkleHasher::key(5);

    // The key computed in `const` context and at runtime must be the same
    assert_eq!(KEY, TransactionsMerkleHasher::key(5));
    assert_eq!(
        KEY,
        ab_blake3::const_keyed_hash(&TransactionsMerkleHasher::BASE_KEY, &5u64.to_le_bytes())
    );
    assert_ne!(KEY, TransactionsMerkleHasher::key(6));

    let hasher = TransactionsMerkleHasher::new_for_tree_size(5);
    test_hasher::<1, _>(&hasher, &KEY);
    test_hasher::<3, _>(&hasher, &KEY);
    test_hasher::<16, _>(&hasher, &KEY);
    test_hasher::<17, _>(&hasher, &KEY);

    // Commits to the tree size, so a single leaf is hashed with the key for one leaf
    let [[leaf, _]] = random_pairs::<1>();
    assert_eq!(
        TransactionsMerkleHasher::new_for_tree_size(1).hash_single_leaf(&leaf),
        ab_blake3::const_keyed_hash(&TransactionsMerkleHasher::key(1), &leaf)
    );
}
