#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]
#![feature(generic_arg_infer)]

use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use ab_merkle_tree::unbalanced_hashed::UnbalancedHashedMerkleTree;
use blake3::OUT_LEN;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use std::mem::MaybeUninit;

#[test]
fn mt_balanced_1_leaves() {
    test_basic::<1>();
}

#[test]
fn mt_balanced_2_leaves() {
    test_basic::<2>();
}

#[test]
fn mt_balanced_4_leaves() {
    test_basic::<4>();
}

#[test]
fn mt_balanced_8_leaves() {
    test_basic::<8>();
}

#[test]
fn mt_balanced_16_leaves() {
    test_basic::<16>();
}

#[test]
fn mt_balanced_32_leaves() {
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
    assert_eq!(
        UnbalancedHashedMerkleTree::compute_root_only(leaves.iter()).unwrap(),
        root
    );

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

    // Ensure the number of proofs (declared and actual) is what it is expected to be
    assert_eq!(tree.all_proofs().len(), N);
    assert_eq!(tree.all_proofs().count(), N);
    assert_eq!(
        tree.all_proofs().fold(0_usize, |acc, _proof| { acc + 1 }),
        N
    );

    let proof_buffer = &mut [MaybeUninit::uninit(); _];

    for (leaf_index, (proof, leaf)) in tree.all_proofs().zip(leaves).enumerate() {
        assert!(
            BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        // Proof is empty for a single leaf and will never fail
        if N > 1 {
            assert!(
                !BalancedHashedMerkleTree::verify(&root, &random_proof, leaf_index, leaf),
                "N {N} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !BalancedHashedMerkleTree::verify(&root, &proof, bad_leaf_index, leaf),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedHashedMerkleTree::verify(&root, &proof, leaf_index, random_hash),
            "N {N} leaf_index {leaf_index}"
        );

        // Ensure unbalanced implementation produces the same proofs and can verify them
        // successfully
        let (unbalanced_root, unbalanced_proof) =
            UnbalancedHashedMerkleTree::<N>::compute_root_and_proof_in(
                leaves.iter(),
                leaf_index,
                proof_buffer,
            )
            .unwrap();
        assert_eq!(unbalanced_root, root, "N {N} leaf_index {leaf_index}");
        assert_eq!(
            proof.as_slice(),
            unbalanced_proof,
            "N {N} leaf_index {leaf_index}"
        );
        #[cfg(feature = "alloc")]
        {
            let (unbalanced_root, unbalanced_proof) =
                UnbalancedHashedMerkleTree::<N>::compute_root_and_proof(leaves.iter(), leaf_index)
                    .unwrap();
            assert_eq!(unbalanced_root, root, "N {N} leaf_index {leaf_index}");
            assert_eq!(
                proof.as_slice(),
                unbalanced_proof.as_slice(),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            UnbalancedHashedMerkleTree::<N>::verify(&root, &proof, leaf_index, leaf, N),
            "N {N} leaf_index {leaf_index}"
        );
        // Proof is empty for a single leaf and will never fail
        if N > 1 {
            assert!(
                !UnbalancedHashedMerkleTree::<N>::verify(&root, &random_proof, leaf_index, leaf, N),
                "N {N} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !UnbalancedHashedMerkleTree::<N>::verify(&root, &proof, bad_leaf_index, leaf, N),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            !UnbalancedHashedMerkleTree::<N>::verify(&root, &proof, leaf_index + 1, leaf, N),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedHashedMerkleTree::<N>::verify(&root, &proof, leaf_index, random_hash, N),
            "N {N} leaf_index {leaf_index}"
        );
    }
}
