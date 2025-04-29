#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]

use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use blake3::OUT_LEN;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

#[test]
fn basic_1() {
    test_basic::<1>();
}

#[test]
fn basic_2() {
    test_basic::<2>();
}

#[test]
fn basic_4() {
    test_basic::<4>();
}

#[test]
fn basic_8() {
    test_basic::<8>();
}

#[test]
fn basic_16() {
    test_basic::<16>();
}

#[test]
fn basic_32() {
    test_basic::<32>();
}

fn test_basic<const N: usize>()
where
    [(); N - 1]:,
    [(); N.ilog2() as usize]:,
    [(); N.ilog2() as usize + 1]:,
{
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = [[0u8; OUT_LEN]; N];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    let tree = BalancedHashedMerkleTree::new(&leaves);
    let root = tree.root();
    #[cfg(feature = "alloc")]
    assert_eq!(BalancedHashedMerkleTree::new_boxed(&leaves).root(), root);

    assert_eq!(BalancedHashedMerkleTree::compute_root_only(&leaves), root);

    let random_hash = {
        let mut hash = [0u8; OUT_LEN];
        rng.fill_bytes(&mut hash);
        hash
    };
    let random_proof = {
        let mut proof = [[0u8; OUT_LEN]; N.ilog2() as usize];
        for hash in &mut proof {
            rng.fill_bytes(hash);
        }
        proof
    };
    for (leaf_index, (proof, leaf)) in tree.all_proofs().zip(leaves).enumerate() {
        assert!(
            BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &random_proof, leaf_index, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, random_hash),
            "N {N} leaf_index {leaf_index}"
        );
    }
}
