#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]

use ab_blake3::OUT_LEN;
use ab_merkle_tree::balanced::{
    BalancedMerkleTree, compute_root_only_large_stack_size, ensure_supported_n,
};
use ab_merkle_tree::mmr::MerkleMountainRange;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::mem::MaybeUninit;

#[test]
fn mt_balanced_2_leaves() {
    test_basic::<2, 2>();
}

#[test]
fn mt_balanced_4_leaves() {
    test_basic::<4, 4>();
}

#[test]
fn mt_balanced_8_leaves() {
    test_basic::<8, 8>();
}

#[test]
fn mt_balanced_16_leaves() {
    test_basic::<16, 16>();
}

#[test]
fn mt_balanced_32_leaves() {
    test_basic::<32, 32>();
}

#[test]
#[cfg(not(feature = "no-panic"))]
#[cfg_attr(miri, ignore)]
fn mt_balanced_64_leaves() {
    test_basic::<64, 64>();
}

fn test_basic<const N: usize, const N_U64: u64>()
where
    [(); N - 1]:,
    [(); ensure_supported_n(N)]:,
    [(); N.ilog2() as usize + 1]:,
    [(); compute_root_only_large_stack_size(N)]:,
    [(); N_U64.next_power_of_two().ilog2() as usize + 1]:,
{
    assert!(N as u64 == N_U64);

    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = [[0u8; OUT_LEN]; N];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    let tree = BalancedMerkleTree::new(&leaves);
    let root = tree.root();
    #[cfg(feature = "alloc")]
    assert_eq!(BalancedMerkleTree::new_boxed(&leaves).root(), root);

    assert_eq!(BalancedMerkleTree::compute_root_only(&leaves), root);
    assert_eq!(
        UnbalancedMerkleTree::compute_root_only::<N_U64, _, _>(leaves.iter().copied()).unwrap(),
        root
    );
    {
        let mut mmr = MerkleMountainRange::<N_U64>::new();
        assert!(mmr.add_leaves(leaves.iter().copied()));
        assert_eq!(mmr.root().unwrap(), root);
    }

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

    let mut mmr = MerkleMountainRange::<N_U64>::new();
    assert_eq!(
        mmr.peaks(),
        MerkleMountainRange::from_peaks(&mmr.peaks())
            .unwrap()
            .peaks()
    );
    let proof_buffer = &mut [MaybeUninit::uninit(); _];

    for (leaf_index, (proof, leaf)) in tree.all_proofs().zip(leaves).enumerate() {
        assert!(
            BalancedMerkleTree::verify(&root, &proof, leaf_index, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        // Proof is empty for a single leaf and will never fail
        if N > 1 {
            assert!(
                !BalancedMerkleTree::verify(&root, &random_proof, leaf_index, leaf),
                "N {N} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !BalancedMerkleTree::verify(&root, &proof, bad_leaf_index, leaf),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            !BalancedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !BalancedMerkleTree::verify(&root, &proof, leaf_index, random_hash),
            "N {N} leaf_index {leaf_index}"
        );

        // Ensure unbalanced implementation produces the same proofs and can verify them
        // successfully
        let (unbalanced_root, unbalanced_proof) =
            UnbalancedMerkleTree::compute_root_and_proof_in::<N_U64, _, _>(
                leaves.iter().copied(),
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
                UnbalancedMerkleTree::compute_root_and_proof::<N_U64, _, _>(
                    leaves.iter().copied(),
                    leaf_index,
                )
                .unwrap();
            assert_eq!(unbalanced_root, root, "N {N} leaf_index {leaf_index}");
            assert_eq!(
                proof.as_slice(),
                unbalanced_proof.as_slice(),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            UnbalancedMerkleTree::verify(&root, &proof, leaf_index as u64, leaf, N_U64),
            "N {N} leaf_index {leaf_index}"
        );
        // Proof is empty for a single leaf and will never fail
        if N > 1 {
            assert!(
                !UnbalancedMerkleTree::verify(&root, &random_proof, leaf_index as u64, leaf, N_U64),
                "N {N} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !UnbalancedMerkleTree::verify(&root, &proof, bad_leaf_index as u64, leaf, N_U64),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            !UnbalancedMerkleTree::verify(&root, &proof, leaf_index as u64 + 1, leaf, N_U64),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedMerkleTree::verify(&root, &proof, leaf_index as u64, random_hash, N_U64),
            "N {N} leaf_index {leaf_index}"
        );

        // Ensure MMR implementation produces the same proofs and can verify them successfully
        let mmr_before = mmr;

        // Add leaves individually with proof generation
        let (expected_mmr_root, expected_mmr_proof) = mmr
            .add_leaf_and_compute_proof_in(&leaf, proof_buffer)
            .unwrap();
        assert_eq!(
            mmr.peaks(),
            MerkleMountainRange::from_peaks(&mmr.peaks())
                .unwrap()
                .peaks(),
            "N {N} leaf_index {leaf_index}"
        );
        assert_eq!(
            mmr.num_leaves(),
            leaf_index as u64 + 1,
            "N {N} leaf_index {leaf_index}"
        );
        assert_eq!(
            mmr.root().unwrap(),
            expected_mmr_root,
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            UnbalancedMerkleTree::verify(
                &expected_mmr_root,
                expected_mmr_proof,
                leaf_index as u64,
                leaf,
                leaf_index as u64 + 1
            ),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            MerkleMountainRange::verify(
                &expected_mmr_root,
                expected_mmr_proof,
                leaf_index as u64,
                leaf,
                leaf_index as u64 + 1
            ),
            "N {N} leaf_index {leaf_index}"
        );

        // Add leaves individually without proof generation
        {
            let mut mmr = mmr_before;
            assert!(mmr.add_leaf(&leaf));
            assert_eq!(
                mmr.num_leaves(),
                leaf_index as u64 + 1,
                "N {N} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.root().unwrap(),
                expected_mmr_root,
                "N {N} leaf_index {leaf_index}"
            );
        }

        // Add leaves individually with proof generation and `alloc`
        #[cfg(feature = "alloc")]
        {
            let mut mmr = mmr_before;
            let (mmr_root, mmr_proof) = mmr.add_leaf_and_compute_proof(&leaf).unwrap();
            assert_eq!(
                mmr.num_leaves(),
                leaf_index as u64 + 1,
                "N {N} leaf_index {leaf_index}"
            );
            assert_eq!(mmr_root, expected_mmr_root, "N {N} leaf_index {leaf_index}");
            assert_eq!(
                mmr_proof.as_slice(),
                expected_mmr_proof,
                "N {N} leaf_index {leaf_index}"
            );
        }

        // Add leaves in bulk without proof generation
        {
            let mut mmr = MerkleMountainRange::new();
            assert!(mmr.add_leaves(leaves.iter().copied().take(leaf_index + 1)));
            assert_eq!(
                mmr.num_leaves(),
                leaf_index as u64 + 1,
                "N {N} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.root().unwrap(),
                expected_mmr_root,
                "N {N} leaf_index {leaf_index}"
            );
        }

        // Add leaves after incremental in bulk without proof generation
        {
            // Add all but last incrementally, then the last in bulk
            {
                let mut mmr = MerkleMountainRange::new();
                for leaf in leaves.iter().take(leaf_index) {
                    assert!(mmr.add_leaf(leaf), "N {N} leaf_index {leaf_index}");
                }
                assert!(mmr.add_leaves(leaves.iter().copied().skip(leaf_index).take(1)));
                assert_eq!(
                    mmr.num_leaves(),
                    leaf_index as u64 + 1,
                    "N {N} leaf_index {leaf_index}"
                );
                assert_eq!(
                    mmr.root().unwrap(),
                    expected_mmr_root,
                    "N {N} leaf_index {leaf_index}"
                );
            }
            // Add one incrementally, then the rest in bulk
            {
                let mut mmr = MerkleMountainRange::new();
                for leaf in leaves.iter().take(1) {
                    assert!(mmr.add_leaf(leaf), "N {N} leaf_index {leaf_index}");
                }
                assert!(mmr.add_leaves(leaves.iter().copied().skip(1).take(leaf_index)));
                assert_eq!(
                    mmr.num_leaves(),
                    leaf_index as u64 + 1,
                    "N {N} leaf_index {leaf_index}"
                );
                assert_eq!(
                    mmr.root().unwrap(),
                    expected_mmr_root,
                    "N {N} leaf_index {leaf_index}"
                );
            }
        }

        assert!(
            MerkleMountainRange::verify(&root, &proof, leaf_index as u64, leaf, N_U64),
            "N {N} leaf_index {leaf_index}"
        );
        // Proof is empty for a single leaf and will never fail
        if N > 1 {
            assert!(
                !MerkleMountainRange::verify(
                    &root,
                    &random_proof,
                    leaf_index as u64,
                    leaf,
                    leaf_index as u64 + 1
                ),
                "N {N} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !MerkleMountainRange::verify(
                    &root,
                    &proof,
                    bad_leaf_index as u64,
                    leaf,
                    leaf_index as u64 + 1
                ),
                "N {N} leaf_index {leaf_index}"
            );
        }
        assert!(
            !MerkleMountainRange::verify(
                &root,
                &proof,
                leaf_index as u64 + 1,
                leaf,
                leaf_index as u64 + 1
            ),
            "N {N} leaf_index {leaf_index}"
        );
        assert!(
            !MerkleMountainRange::verify(
                &root,
                &proof,
                leaf_index as u64,
                random_hash,
                leaf_index as u64 + 1
            ),
            "N {N} leaf_index {leaf_index}"
        );

        if leaf_index == leaves.len() - 1 {
            assert_eq!(root, expected_mmr_root, "N {N} leaf_index {leaf_index}");
            assert_eq!(proof, expected_mmr_proof, "N {N} leaf_index {leaf_index}");
        }
    }
}
