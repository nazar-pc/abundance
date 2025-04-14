#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]

use ab_merkle_tree::balanced_hashed::{BalancedHashedMerkleTree, num_hashes, num_leaves};
use blake3::OUT_LEN;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

#[test]
fn basic_0() {
    test_basic::<0>();
}

#[test]
fn basic_1() {
    test_basic::<1>();
}

#[test]
fn basic_2() {
    test_basic::<2>();
}

#[test]
fn basic_3() {
    test_basic::<3>();
}

#[test]
fn basic_4() {
    test_basic::<4>();
}

#[test]
fn basic_5() {
    test_basic::<5>();
}

fn test_basic<const NUM_LEAVES_LOG_2: u32>()
where
    [(); NUM_LEAVES_LOG_2 as usize]:,
    [(); num_leaves(NUM_LEAVES_LOG_2)]:,
    [(); num_hashes(NUM_LEAVES_LOG_2)]:,
{
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaf_hashes = {
        let mut leaf_hashes = [[0u8; OUT_LEN]; num_leaves(NUM_LEAVES_LOG_2)];
        for hash in &mut leaf_hashes {
            rng.fill_bytes(hash);
        }
        leaf_hashes
    };

    let tree = BalancedHashedMerkleTree::new(&leaf_hashes);
    let root = tree.root();
    #[cfg(feature = "alloc")]
    assert_eq!(
        BalancedHashedMerkleTree::new_boxed(&leaf_hashes).root(),
        root
    );

    let random_hash = {
        let mut hash = [0u8; OUT_LEN];
        rng.fill_bytes(&mut hash);
        hash
    };
    let random_proof = {
        let mut proof = [[0u8; OUT_LEN]; NUM_LEAVES_LOG_2 as usize];
        for hash in &mut proof {
            rng.fill_bytes(hash);
        }
        proof
    };
    for (leaf_index, (proof, leaf_hash)) in tree.all_proofs().zip(leaf_hashes).enumerate().skip(2) {
        assert!(
            BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, leaf_hash),
            "num_leaves_log_2 {NUM_LEAVES_LOG_2} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, random_hash),
            "num_leaves_log_2 {NUM_LEAVES_LOG_2} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &random_proof, leaf_index, leaf_hash),
            "num_leaves_log_2 {NUM_LEAVES_LOG_2} leaf_index {leaf_index}"
        );
    }
}
