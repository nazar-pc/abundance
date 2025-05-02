#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]
#![feature(generic_arg_infer)]

use ab_merkle_tree::hash_pair;
use ab_merkle_tree::unbalanced_hashed::UnbalancedHashedMerkleTree;
use blake3::OUT_LEN;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use std::mem;
use std::mem::MaybeUninit;

const MAX_N: usize = 100;

/// A simplified version that is easier to audit to verify the main optimized version against
struct SimpleUnbalancedMerkleTree;

impl SimpleUnbalancedMerkleTree {
    /// Compute Merkle Tree Root
    fn compute_root_only<'a, Iter>(leaves: Iter) -> Option<[u8; OUT_LEN]>
    where
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        let mut nodes = leaves.cloned().collect::<Vec<[u8; OUT_LEN]>>();
        if nodes.is_empty() {
            return None;
        }
        if nodes.len() == 1 {
            return Some(nodes[0]);
        }

        // Build the tree level by level
        let mut next_level = Vec::with_capacity(nodes.len().div_ceil(2));
        while nodes.len() > 1 {
            for i in (0..nodes.len()).step_by(2) {
                if i + 1 < nodes.len() {
                    // Hash two nodes together
                    next_level.push(hash_pair(&nodes[i], &nodes[i + 1]));
                } else {
                    // Promote the last node as is
                    next_level.push(nodes[i]);
                }
            }
            mem::swap(&mut nodes, &mut next_level);
            next_level.clear();
        }
        Some(nodes[0])
    }

    /// Compute Merkle Tree Root and generate a proof for the leaf at target_index
    fn compute_root_and_proof<'a, Iter>(
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; OUT_LEN], Vec<[u8; OUT_LEN]>)>
    where
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        let mut nodes = leaves.cloned().collect::<Vec<[u8; OUT_LEN]>>();
        if nodes.is_empty() || target_index >= nodes.len() {
            return None;
        }

        let mut proof = Vec::new();
        let mut current_index = target_index;

        // Build the tree and collect proof
        let mut next_level = Vec::with_capacity(nodes.len().div_ceil(2));
        while nodes.len() > 1 {
            for i in (0..nodes.len()).step_by(2) {
                if i + 1 < nodes.len() {
                    // Hash two nodes
                    let parent = hash_pair(&nodes[i], &nodes[i + 1]);
                    next_level.push(parent);

                    // Add sibling to proof if this pair includes the target
                    if current_index == i {
                        // Right sibling
                        proof.push(nodes[i + 1]);
                    } else if current_index == i + 1 {
                        // Left sibling
                        proof.push(nodes[i]);
                    }
                } else {
                    // Promote the last node
                    next_level.push(nodes[i]);
                }
            }
            // Update index for the next level
            current_index /= 2;
            mem::swap(&mut nodes, &mut next_level);
            next_level.clear();
        }
        Some((nodes[0], proof))
    }

    /// Verify a Merkle proof for a leaf at the given index
    fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]],
        leaf_index: usize,
        leaf: [u8; OUT_LEN],
        num_leaves: usize,
    ) -> bool {
        if leaf_index >= num_leaves {
            return false;
        }

        let mut current = leaf;
        let mut index = leaf_index;
        let mut proof_pos = 0;
        let mut level_size = num_leaves;

        // Rebuild the path to the root
        while level_size > 1 {
            let is_left = index % 2 == 0;
            let is_last = index == level_size - 1;

            if is_left && !is_last {
                // Left node with a right sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hash_pair(&current, &proof[proof_pos]);
                proof_pos += 1;
            } else if !is_left {
                // Right node with a left sibling
                if proof_pos >= proof.len() {
                    // Missing sibling
                    return false;
                }
                current = hash_pair(&proof[proof_pos], &current);
                proof_pos += 1;
            } else {
                // Last node, no sibling, keep current
            }

            index /= 2;
            // Size of next level
            level_size = level_size.div_ceil(2);
        }

        // Check if proof is fully used and matches root
        proof_pos == proof.len() && current == *root
    }
}

#[test]
fn mt_unbalanced_3_leaves() {
    test_basic(3);
}

#[test]
fn mt_unbalanced_5_leaves() {
    test_basic(5);
}

#[test]
fn mt_unbalanced_6_leaves() {
    test_basic(6);
}

#[test]
fn mt_unbalanced_7_leaves() {
    test_basic(7);
}

#[test]
fn mt_unbalanced_8_leaves() {
    test_basic(8);
}

#[test]
fn mt_unbalanced_9_leaves() {
    test_basic(9);
}

#[test]
fn mt_unbalanced_10_leaves() {
    test_basic(10);
}

#[test]
fn mt_unbalanced_11_leaves() {
    test_basic(11);
}

#[test]
fn mt_unbalanced_12_leaves() {
    test_basic(12);
}

#[test]
fn mt_unbalanced_13_leaves() {
    test_basic(13);
}

#[test]
fn mt_unbalanced_14_leaves() {
    test_basic(14);
}

#[test]
fn mt_unbalanced_15_leaves() {
    test_basic(15);
}

#[test]
#[cfg_attr(miri, ignore)]
fn mt_unbalanced_large_range() {
    for number_of_leaves in 2..MAX_N {
        test_basic(number_of_leaves);
    }
}

fn test_basic(number_of_leaves: usize) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = vec![[0u8; OUT_LEN]; number_of_leaves];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    let root = SimpleUnbalancedMerkleTree::compute_root_only(leaves.iter()).unwrap();
    let computed_root =
        UnbalancedHashedMerkleTree::compute_root_only::<'_, MAX_N, _>(leaves.iter()).unwrap();

    assert_eq!(root, computed_root, "number_of_leaves {number_of_leaves}");

    let random_hash = {
        let mut hash = [0u8; OUT_LEN];
        rng.fill_bytes(&mut hash);
        hash
    };
    let random_proof = {
        let mut proof = vec![[0u8; OUT_LEN]; number_of_leaves.ilog2() as usize];
        for hash in &mut proof {
            rng.fill_bytes(hash);
        }
        proof
    };

    let proof_buffer = &mut [MaybeUninit::uninit(); _];

    for (leaf_index, leaf) in leaves.iter().copied().enumerate() {
        let (computed_root, proof) =
            SimpleUnbalancedMerkleTree::compute_root_and_proof(leaves.iter(), leaf_index).unwrap();
        assert_eq!(
            computed_root, root,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        let (computed_root, computed_proof) =
            UnbalancedHashedMerkleTree::compute_root_and_proof_in::<MAX_N, _>(
                leaves.iter(),
                leaf_index,
                proof_buffer,
            )
            .unwrap();
        assert_eq!(
            computed_root, root,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert_eq!(
            computed_proof, proof,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        #[cfg(feature = "alloc")]
        {
            let (computed_root, computed_proof) =
                UnbalancedHashedMerkleTree::compute_root_and_proof::<MAX_N, _>(
                    leaves.iter(),
                    leaf_index,
                )
                .unwrap();
            assert_eq!(
                computed_root, root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                computed_proof, proof,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        assert!(
            SimpleUnbalancedMerkleTree::verify(&root, &proof, leaf_index, leaf, leaves.len()),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            UnbalancedHashedMerkleTree::verify(&root, &proof, leaf_index, leaf, leaves.len()),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        assert!(
            !SimpleUnbalancedMerkleTree::verify(
                &root,
                &random_proof,
                leaf_index,
                leaf,
                leaves.len()
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedHashedMerkleTree::verify(
                &root,
                &random_proof,
                leaf_index,
                leaf,
                leaves.len()
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !SimpleUnbalancedMerkleTree::verify(
                    &root,
                    &proof,
                    bad_leaf_index,
                    leaf,
                    leaves.len()
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert!(
                !UnbalancedHashedMerkleTree::verify(
                    &root,
                    &proof,
                    bad_leaf_index,
                    leaf,
                    leaves.len()
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        assert!(
            !SimpleUnbalancedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf, leaves.len()),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedHashedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf, leaves.len()),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        assert!(
            !SimpleUnbalancedMerkleTree::verify(
                &root,
                &proof,
                leaf_index,
                random_hash,
                leaves.len()
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedHashedMerkleTree::verify(
                &root,
                &proof,
                leaf_index,
                random_hash,
                leaves.len()
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
    }

    assert!(
        UnbalancedHashedMerkleTree::compute_root_and_proof_in::<MAX_N, _>(
            leaves.iter(),
            leaves.len(),
            proof_buffer
        )
        .is_none()
    );
    #[cfg(feature = "alloc")]
    assert!(
        UnbalancedHashedMerkleTree::compute_root_and_proof::<MAX_N, _>(
            leaves.iter(),
            leaves.len(),
        )
        .is_none()
    );
}
