//! BLAKE3 hasher implementation, independently of trees

use ab_merkle_tree::MerkleHasher;
use ab_merkle_tree::blake3::Blake3MerkleHasher;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};

fn random_pairs<H, const NUM_PAIRS: usize>() -> [[[u8; H::OUT_LEN]; 2]; NUM_PAIRS]
where
    H: MerkleHasher,
{
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut pairs = [[[0u8; H::OUT_LEN]; 2]; NUM_PAIRS];
    for node in pairs.as_flattened_mut() {
        rng.fill_bytes(node);
    }
    pairs
}

fn test_hash_pairs<const NUM_PAIRS: usize>() {
    let hasher = Blake3MerkleHasher;
    let pairs = random_pairs::<Blake3MerkleHasher, NUM_PAIRS>();

    assert_eq!(
        hasher.hash_pairs(&pairs),
        pairs.each_ref().map(|pair| hasher.hash_pair(pair)),
        "NUM_PAIRS {NUM_PAIRS}"
    );
}

#[test]
fn blake3_hash_pair() {
    let hasher = Blake3MerkleHasher;

    for pair in random_pairs::<Blake3MerkleHasher, 4>() {
        assert_eq!(
            hasher.hash_pair(&pair),
            ab_blake3::const_keyed_hash(&Blake3MerkleHasher::KEY, pair.as_flattened())
        );
    }
}

#[test]
fn blake3_hash_pairs() {
    test_hash_pairs::<1>();
    test_hash_pairs::<2>();
    test_hash_pairs::<3>();
    test_hash_pairs::<4>();
    test_hash_pairs::<8>();
    test_hash_pairs::<16>();
    test_hash_pairs::<17>();
    test_hash_pairs::<32>();
}
