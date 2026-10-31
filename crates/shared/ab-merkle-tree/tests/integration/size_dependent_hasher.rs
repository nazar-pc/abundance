//! Trees that know the number of leaves upfront support hashers that depend on the tree size

use crate::test_hasher::{OUT_LEN, TestHasher, TreeSizeTestHasher};
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::iter;
use std::mem::MaybeUninit;

type Balanced<'a, const N: usize> = BalancedMerkleTree<'a, N, TreeSizeTestHasher>;
type Unbalanced = UnbalancedMerkleTree<TreeSizeTestHasher>;
type SizeIndependent = UnbalancedMerkleTree<TestHasher>;

fn random_leaves<const N: usize>() -> [[u8; OUT_LEN]; N] {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut leaves = [[0u8; OUT_LEN]; N];
    for hash in &mut leaves {
        rng.fill_bytes(hash);
    }
    leaves
}

/// Root of a balanced tree, hashing one level at a time
fn reference_root(hasher: TreeSizeTestHasher, leaves: &[[u8; OUT_LEN]]) -> [u8; OUT_LEN] {
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        level = level
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| hasher.hash_pair(pair))
            .collect();
    }
    level[0]
}

fn test_balanced<const N: usize>() {
    let leaves = random_leaves::<N>();
    let tree = Balanced::<N>::new(&leaves);
    let root = tree.root();

    assert_eq!(Balanced::<N>::compute_root_only(&leaves), root, "N {N}");
    assert_eq!(
        Unbalanced::compute_root_only::<1024, _, _>(leaves.iter().copied()),
        Some(root),
        "N {N}"
    );
    assert_eq!(Unbalanced::compute_root_only_array(&leaves), root, "N {N}");
    // The hasher is created for the number of leaves in the tree
    let hasher = TreeSizeTestHasher::new_for_tree_size(N as u64);
    assert_eq!(reference_root(hasher, &leaves), root, "N {N}");
    let hasher = TreeSizeTestHasher::new_for_tree_size(N as u64 + 1);
    assert_ne!(reference_root(hasher, &leaves), root, "N {N}");

    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    for (leaf_index, (proof, leaf)) in tree.all_proofs().zip(leaves).enumerate() {
        assert!(
            Balanced::<N>::verify(&root, &proof, leaf_index, leaf),
            "N {N} leaf_index {leaf_index}"
        );

        let (unbalanced_root, unbalanced_proof) =
            Unbalanced::compute_root_and_proof_in::<1024, _, _>(
                leaves.iter().copied(),
                leaf_index,
                proof_buffer,
            )
            .unwrap();
        assert_eq!(unbalanced_root, root, "N {N} leaf_index {leaf_index}");
        assert_eq!(unbalanced_proof, proof, "N {N} leaf_index {leaf_index}");
        assert!(
            Unbalanced::verify(&root, &proof, leaf_index as u64, leaf, N as u64),
            "N {N} leaf_index {leaf_index}"
        );
    }
}

#[test]
fn size_dependent_hasher_balanced() {
    test_balanced::<2>();
    test_balanced::<4>();
    test_balanced::<16>();
    // The smallest tree that uses batched hashing
    test_balanced::<32>();
}

#[test]
#[cfg(not(any(feature = "no-panic", miri)))]
fn size_dependent_hasher_balanced_large() {
    test_balanced::<1024>();
}

/// A proof for the first leaf in a tree with 5 leaves has the same shape as one in a tree with 6
/// leaves, but verifying it with 6 leaves uses a hasher created for 6 leaves, so the root doesn't
/// match
#[test]
fn size_dependent_hasher_rejects_wrong_num_leaves() {
    let leaves = random_leaves::<5>();

    let root = Unbalanced::compute_root_only_array(&leaves);
    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    let (proof_root, proof) =
        Unbalanced::compute_root_and_proof_in::<5, _, _>(leaves.iter().copied(), 0, proof_buffer)
            .unwrap();
    assert_eq!(proof_root, root);

    assert!(Unbalanced::verify(&root, proof, 0, leaves[0], 5));
    assert!(!Unbalanced::verify(&root, proof, 0, leaves[0], 6));

    // A hasher that ignores the tree size accepts a proof of the same shape with the wrong number
    // of leaves, so the rejection above comes from the hasher
    let root = SizeIndependent::compute_root_only_array(&leaves);
    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    let (proof_root, proof) = SizeIndependent::compute_root_and_proof_in::<5, _, _>(
        leaves.iter().copied(),
        0,
        proof_buffer,
    )
    .unwrap();
    assert_eq!(proof_root, root);

    assert!(SizeIndependent::verify(&root, proof, 0, leaves[0], 5));
    assert!(SizeIndependent::verify(&root, proof, 0, leaves[0], 6));
}

/// The root of a tree with a single leaf is the leaf hashed by the hasher, so the root of a larger
/// tree can't pass for a single leaf
#[test]
fn size_dependent_hasher_single_leaf() {
    let leaves = random_leaves::<2>();
    let [leaf, _] = leaves;

    let root = Unbalanced::compute_root_only_array(&[leaf]);
    assert_eq!(
        root,
        TreeSizeTestHasher::new_for_tree_size(1).hash_single_leaf(&leaf)
    );

    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    let (proof_root, proof) =
        Unbalanced::compute_root_and_proof_in::<1, _, _>(iter::once(leaf), 0, proof_buffer)
            .unwrap();
    assert_eq!(proof_root, root);
    assert!(proof.is_empty());
    assert!(Unbalanced::verify(&root, proof, 0, leaf, 1));

    // The root of two leaves claimed to be the only leaf
    let root = Unbalanced::compute_root_only_array(&leaves);
    assert!(!Unbalanced::verify(&root, &[], 0, root, 1));

    // A hasher that keeps a single leaf unchanged accepts it, so the rejection above comes from the
    // hasher
    let root = SizeIndependent::compute_root_only_array(&leaves);
    assert!(SizeIndependent::verify(&root, &[], 0, root, 1));
}

/// `TrustedLen` iterators of unsupported length are rejected
#[test]
fn size_dependent_hasher_iterator_length() {
    let leaves = random_leaves::<5>();
    let root = Unbalanced::compute_root_only_array(&leaves);

    // Adapters of `TrustedLen` iterators are `TrustedLen` too
    assert_eq!(
        Unbalanced::compute_root_only::<5, _, _>(
            iter::once(leaves[0]).chain(leaves[1..].iter().copied())
        ),
        Some(root)
    );
    assert_eq!(
        Unbalanced::compute_root_only::<4, _, _>(leaves.iter().copied()),
        None
    );
    assert_eq!(
        Unbalanced::compute_root_only::<5, _, _>(iter::empty::<[u8; OUT_LEN]>()),
        None
    );
    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    assert_eq!(
        Unbalanced::compute_root_and_proof_in::<4, _, _>(leaves.iter().copied(), 0, proof_buffer),
        None
    );
}
