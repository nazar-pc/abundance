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

// A function generic over the hasher would need `[u8; H::OUT_LEN]`, which this crate doesn't
// support (see `impl_merkle_hasher!`)
macro_rules! test_hasher {
    ($hasher:expr, $key:expr, $($num_pairs:literal),+) => {
        $({
            let hasher = &$hasher;
            let pairs = random_pairs::<$num_pairs>();
            let hashes = pairs.each_ref().map(|pair| hasher.hash_pair(pair));

            for (pair, hash) in pairs.iter().zip(&hashes) {
                assert_eq!(
                    *hash,
                    ab_blake3::const_keyed_hash($key, pair.as_flattened()),
                    "NUM_PAIRS {}",
                    $num_pairs
                );
            }
            assert_eq!(hasher.hash_pairs(&pairs), hashes, "NUM_PAIRS {}", $num_pairs);
        })+
    };
}

#[test]
fn static_hasher() {
    test_hasher!(
        BlockMerkleMountainRangeHasher::default(),
        &BlockMerkleMountainRangeHasher::KEY,
        1,
        3,
        16,
        17
    );

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

    test_hasher!(
        TransactionsMerkleHasher::new_for_tree_size(5),
        &KEY,
        1,
        3,
        16,
        17
    );

    // Commits to the tree size, so a single leaf is hashed with the key for one leaf
    let [[leaf, _]] = random_pairs::<1>();
    assert_eq!(
        TransactionsMerkleHasher::new_for_tree_size(1).hash_single_leaf(&leaf),
        ab_blake3::const_keyed_hash(&TransactionsMerkleHasher::key(1), &leaf)
    );
}
