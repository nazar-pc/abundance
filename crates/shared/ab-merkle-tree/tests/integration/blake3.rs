//! BLAKE3 hasher implementation, independently of trees

use ab_blake3::KEY_LEN;
use ab_merkle_tree::MerkleHasher;
use ab_merkle_tree::blake3::{Blake3MerkleHasher, Blake3MerkleKey, DefaultBlake3MerkleKey};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::any::type_name;

#[derive(Debug)]
struct CustomKey;

impl Blake3MerkleKey for CustomKey {
    const KEY: [u8; KEY_LEN] = [7; KEY_LEN];
}

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

fn test_hash_pairs<K, const NUM_PAIRS: usize>()
where
    K: Blake3MerkleKey,
{
    let hasher = Blake3MerkleHasher::<K>::default();
    let pairs = random_pairs::<Blake3MerkleHasher<K>, NUM_PAIRS>();

    assert_eq!(
        hasher.hash_pairs(&pairs),
        pairs.each_ref().map(|pair| hasher.hash_pair(pair)),
        "K {} NUM_PAIRS {NUM_PAIRS}",
        type_name::<K>()
    );
}

#[test]
fn blake3_hash_pair() {
    let default_hasher = const { Blake3MerkleHasher::<DefaultBlake3MerkleKey>::default() };
    let custom_hasher = const { Blake3MerkleHasher::<CustomKey>::default() };

    for pair in random_pairs::<Blake3MerkleHasher, 4>() {
        let input = pair.as_flattened();

        assert_eq!(
            default_hasher.hash_pair(&pair),
            ab_blake3::const_keyed_hash(&DefaultBlake3MerkleKey::KEY, input)
        );
        assert_eq!(
            custom_hasher.hash_pair(&pair),
            ab_blake3::const_keyed_hash(&CustomKey::KEY, input)
        );
    }
}

#[test]
fn blake3_hash_pairs() {
    test_hash_pairs::<DefaultBlake3MerkleKey, 1>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 2>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 3>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 4>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 8>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 16>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 17>();
    test_hash_pairs::<DefaultBlake3MerkleKey, 32>();

    test_hash_pairs::<CustomKey, 1>();
    test_hash_pairs::<CustomKey, 2>();
    test_hash_pairs::<CustomKey, 3>();
    test_hash_pairs::<CustomKey, 4>();
    test_hash_pairs::<CustomKey, 8>();
    test_hash_pairs::<CustomKey, 16>();
    test_hash_pairs::<CustomKey, 17>();
    test_hash_pairs::<CustomKey, 32>();
}
