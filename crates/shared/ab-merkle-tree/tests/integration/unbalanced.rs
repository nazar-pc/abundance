use crate::test_hasher::{
    MerkleMountainRange, OUT_LEN, TreeSizeTestHasher, UnbalancedMerkleTree, hash_pair,
};
use ab_merkle_tree::{MerkleHasher, NewMerkleHasherForTreeSize, unbalanced};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{iter, mem};

const MAX_N: u64 = 100;

/// Tree with a hasher that depends on the number of leaves, so that the sparse root is checked to
/// use the number of leaves of the whole tree
type TreeSizeUnbalanced = unbalanced::UnbalancedMerkleTree<TreeSizeTestHasher>;

/// Iterator over `leaves` that is not `TrustedLen`
fn not_trusted_len(leaves: &[[u8; OUT_LEN]]) -> impl Iterator<Item = [u8; OUT_LEN]> + '_ {
    let mut leaves = leaves.iter().copied();
    iter::from_fn(move || leaves.next())
}

/// A simplified version that is easier to audit to verify the main optimized version against
struct SimpleUnbalancedMerkleTree;

impl SimpleUnbalancedMerkleTree {
    /// Compute Merkle Tree Root
    fn compute_root_only<'a, Iter>(leaves: Iter) -> Option<[u8; OUT_LEN]>
    where
        Iter: Iterator<Item = &'a [u8; OUT_LEN]> + 'a,
    {
        let mut nodes = leaves.copied().collect::<Vec<[u8; OUT_LEN]>>();
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
    fn compute_root_and_proof<'a, Item, Iter>(
        leaves: Iter,
        target_index: usize,
    ) -> Option<([u8; OUT_LEN], Vec<[u8; OUT_LEN]>)>
    where
        Item: Into<[u8; OUT_LEN]>,
        Iter: IntoIterator<Item = Item> + 'a,
    {
        let mut nodes = leaves.into_iter().map(Into::into).collect::<Vec<_>>();
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
        let mut position = leaf_index;
        let mut proof_pos = 0;
        let mut level_size = num_leaves;

        // Rebuild the path to the root
        while level_size > 1 {
            let is_left = position.is_multiple_of(2);
            let is_last = position == level_size - 1;

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

            position /= 2;
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
fn mt_unbalanced_too_many_leaves() {
    const NUM_LEAVES: u64 = 3;
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = vec![[0u8; OUT_LEN]; NUM_LEAVES as usize + 1];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    assert!(
        UnbalancedMerkleTree::compute_root_only::<NUM_LEAVES, _, _>(leaves.iter().copied())
            .is_none()
    );
    assert!(
        UnbalancedMerkleTree::compute_root_only_dynamic_len::<NUM_LEAVES, _, _>(not_trusted_len(
            &leaves
        ))
        .is_none()
    );
    let proof_buffer = &mut [MaybeUninit::uninit(); _];
    assert!(
        UnbalancedMerkleTree::compute_root_and_proof_in::<NUM_LEAVES, _, _>(
            leaves.iter().copied(),
            0,
            proof_buffer
        )
        .is_none()
    );
    assert!(
        UnbalancedMerkleTree::compute_root_and_proof_dynamic_len_in::<NUM_LEAVES, _, _>(
            not_trusted_len(&leaves),
            0,
            proof_buffer
        )
        .is_none()
    );
    #[cfg(feature = "alloc")]
    {
        assert!(
            UnbalancedMerkleTree::compute_root_and_proof::<NUM_LEAVES, _, _>(
                leaves.iter().copied(),
                0
            )
            .is_none()
        );
        assert!(
            UnbalancedMerkleTree::compute_root_and_proof_dynamic_len::<NUM_LEAVES, _, _>(
                not_trusted_len(&leaves),
                0
            )
            .is_none()
        );
    }
}

#[test]
#[cfg(not(miri))]
fn mt_unbalanced_large_range() {
    for number_of_leaves in 1..MAX_N {
        test_basic(number_of_leaves);
    }
}

#[test]
fn mt_unbalanced_array() {
    test_array::<1>();
    test_array::<2>();
    test_array::<3>();
    test_array::<5>();
    test_array::<8>();
    test_array::<13>();
}

fn test_array<const N: usize>() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let mut leaves = [[0u8; OUT_LEN]; N];
    for hash in &mut leaves {
        rng.fill_bytes(hash);
    }

    let root = SimpleUnbalancedMerkleTree::compute_root_only(leaves.iter()).unwrap();
    let computed_root = UnbalancedMerkleTree::compute_root_only_array(&leaves);

    assert_eq!(root, computed_root, "N {N}");
}

fn test_basic(number_of_leaves: u64) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = vec![[0u8; OUT_LEN]; number_of_leaves as usize];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    let root = SimpleUnbalancedMerkleTree::compute_root_only(leaves.iter()).unwrap();
    let computed_root =
        UnbalancedMerkleTree::compute_root_only::<'_, MAX_N, _, _>(leaves.iter().copied()).unwrap();

    assert_eq!(root, computed_root, "number_of_leaves {number_of_leaves}");
    assert_eq!(
        UnbalancedMerkleTree::compute_root_only_dynamic_len::<MAX_N, _, _>(not_trusted_len(
            &leaves
        )),
        Some(root),
        "number_of_leaves {number_of_leaves}"
    );

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

    // Merkle Mountain Range aggregates leaves incrementally, so after adding `leaf_index + 1`
    // leaves it represents a tree of that many leaves, whose root and proofs must match those
    // produced by the other implementations for the same prefix of leaves.
    let mut mmr = MerkleMountainRange::<MAX_N>::new();
    assert_eq!(
        mmr.peaks(),
        MerkleMountainRange::from_peaks(&mmr.peaks())
            .unwrap()
            .peaks()
    );
    let proof_buffer_mmr = &mut [MaybeUninit::uninit(); _];

    for (leaf_index, leaf) in leaves.iter().copied().enumerate() {
        let (computed_root, proof) =
            SimpleUnbalancedMerkleTree::compute_root_and_proof(leaves.iter().copied(), leaf_index)
                .unwrap();
        assert_eq!(
            computed_root, root,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        let (computed_root, computed_proof) =
            UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
                leaves.iter().copied(),
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
        {
            let proof_buffer = &mut [MaybeUninit::uninit(); _];
            let (computed_root, computed_proof) =
                UnbalancedMerkleTree::compute_root_and_proof_dynamic_len_in::<MAX_N, _, _>(
                    not_trusted_len(&leaves),
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
        }
        #[cfg(feature = "alloc")]
        {
            let (computed_root, computed_proof) = UnbalancedMerkleTree::compute_root_and_proof::<
                MAX_N,
                _,
                _,
            >(leaves.iter().copied(), leaf_index)
            .unwrap();
            assert_eq!(
                computed_root, root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                computed_proof, proof,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );

            let (computed_root, computed_proof) =
                UnbalancedMerkleTree::compute_root_and_proof_dynamic_len::<MAX_N, _, _>(
                    not_trusted_len(&leaves),
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
            UnbalancedMerkleTree::verify(
                &root,
                &proof,
                leaf_index as u64,
                leaf,
                leaves.len() as u64
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        if leaves.len() > 1 {
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
                !UnbalancedMerkleTree::verify(
                    &root,
                    &random_proof,
                    leaf_index as u64,
                    leaf,
                    leaves.len() as u64
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        } else {
            // For a single leaf the proof must be empty
            assert!(
                !SimpleUnbalancedMerkleTree::verify(
                    &root,
                    &[[0; OUT_LEN]],
                    leaf_index,
                    leaf,
                    leaves.len()
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert!(
                !UnbalancedMerkleTree::verify(
                    &root,
                    &[[0; OUT_LEN]],
                    leaf_index as u64,
                    leaf,
                    leaves.len() as u64
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

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
                !UnbalancedMerkleTree::verify(
                    &root,
                    &proof,
                    bad_leaf_index as u64,
                    leaf,
                    leaves.len() as u64
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        assert!(
            !SimpleUnbalancedMerkleTree::verify(&root, &proof, leaf_index + 1, leaf, leaves.len()),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            !UnbalancedMerkleTree::verify(
                &root,
                &proof,
                leaf_index as u64 + 1,
                leaf,
                leaves.len() as u64
            ),
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
            !UnbalancedMerkleTree::verify(
                &root,
                &proof,
                leaf_index as u64,
                random_hash,
                leaves.len() as u64
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        // Ensure MMR implementation produces the same root and proofs as the other implementations
        // and can verify them successfully
        let mmr_before = mmr;
        let num_leaves = leaf_index as u64 + 1;
        let partial_root =
            SimpleUnbalancedMerkleTree::compute_root_only(leaves[..=leaf_index].iter()).unwrap();

        // Add leaf individually with proof generation
        let (mmr_root, mmr_proof) = mmr
            .add_leaf_and_compute_proof_in(&leaf, proof_buffer_mmr)
            .unwrap();
        assert_eq!(
            mmr.peaks(),
            MerkleMountainRange::from_peaks(&mmr.peaks())
                .unwrap()
                .peaks(),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert_eq!(
            mmr.num_leaves(),
            num_leaves,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert_eq!(
            mmr_root, partial_root,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert_eq!(
            mmr.root().unwrap(),
            mmr_root,
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            UnbalancedMerkleTree::verify(&mmr_root, mmr_proof, leaf_index as u64, leaf, num_leaves),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        assert!(
            MerkleMountainRange::<MAX_N>::verify(
                &mmr_root,
                mmr_proof,
                leaf_index as u64,
                leaf,
                num_leaves
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );

        // Add leaf individually without proof generation
        {
            let mut mmr = mmr_before;
            assert!(
                mmr.add_leaf(&leaf),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.num_leaves(),
                num_leaves,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.root().unwrap(),
                mmr_root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        // Add leaf individually with proof generation and `alloc`
        #[cfg(feature = "alloc")]
        {
            let mut mmr = mmr_before;
            let (alloc_root, alloc_proof) = mmr.add_leaf_and_compute_proof(&leaf).unwrap();
            assert_eq!(
                mmr.num_leaves(),
                num_leaves,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                alloc_root, mmr_root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                alloc_proof.as_slice(),
                mmr_proof,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        // Add leaves in bulk from scratch
        {
            let mut mmr = MerkleMountainRange::<MAX_N>::new();
            assert!(
                mmr.add_leaves(leaves.iter().copied().take(leaf_index + 1)),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.num_leaves(),
                num_leaves,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.root().unwrap(),
                mmr_root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        // Add leaves incrementally, then the rest in bulk
        {
            let mut mmr = MerkleMountainRange::<MAX_N>::new();
            for leaf in leaves.iter().take(leaf_index) {
                assert!(
                    mmr.add_leaf(leaf),
                    "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
                );
            }
            assert!(
                mmr.add_leaves(leaves.iter().copied().skip(leaf_index).take(1)),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.num_leaves(),
                num_leaves,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr.root().unwrap(),
                mmr_root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        // MMR proof must not verify with a tampered leaf, index or proof
        assert!(
            !MerkleMountainRange::<MAX_N>::verify(
                &mmr_root,
                mmr_proof,
                leaf_index as u64,
                random_hash,
                num_leaves
            ),
            "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
        );
        if num_leaves > 1 {
            assert!(
                !MerkleMountainRange::<MAX_N>::verify(
                    &mmr_root,
                    &random_proof,
                    leaf_index as u64,
                    leaf,
                    num_leaves
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }
        if let Some(bad_leaf_index) = leaf_index.checked_sub(1) {
            assert!(
                !MerkleMountainRange::<MAX_N>::verify(
                    &mmr_root,
                    mmr_proof,
                    bad_leaf_index as u64,
                    leaf,
                    num_leaves
                ),
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }

        // The final MMR state covers all leaves, so its root and the proof for the last leaf must
        // match the full tree computed by the unbalanced implementation
        if leaf_index == leaves.len() - 1 {
            assert_eq!(
                mmr_root, root,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr_proof, computed_proof,
                "number_of_leaves {number_of_leaves} leaf_index {leaf_index}"
            );
        }
    }

    assert!(
        UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
            leaves.iter().copied(),
            leaves.len(),
            proof_buffer
        )
        .is_none()
    );
    #[cfg(feature = "alloc")]
    assert!(
        UnbalancedMerkleTree::compute_root_and_proof::<MAX_N, _, _>(
            leaves.iter().copied(),
            leaves.len(),
        )
        .is_none()
    );

    let empty: [[u8; OUT_LEN]; 0] = [];
    assert!(UnbalancedMerkleTree::compute_root_only::<MAX_N, _, _>(empty).is_none());
    assert!(
        UnbalancedMerkleTree::compute_root_only_dynamic_len::<MAX_N, _, _>(not_trusted_len(&empty))
            .is_none()
    );
    assert!(
        UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(empty, 0, proof_buffer)
            .is_none()
    );
    assert!(
        UnbalancedMerkleTree::compute_root_and_proof_dynamic_len_in::<MAX_N, _, _>(
            not_trusted_len(&empty),
            0,
            proof_buffer
        )
        .is_none()
    );
    #[cfg(feature = "alloc")]
    {
        assert!(UnbalancedMerkleTree::compute_root_and_proof::<MAX_N, _, _>(empty, 0).is_none());
        assert!(
            UnbalancedMerkleTree::compute_root_and_proof_dynamic_len::<MAX_N, _, _>(
                not_trusted_len(&empty),
                0
            )
            .is_none()
        );
    }
}

/// Number of hashes computed by [`CountingTestHasher`]
static NUM_HASHES: AtomicU32 = AtomicU32::new(0);

/// [`TreeSizeTestHasher`] that counts hashes in [`NUM_HASHES`], only one test uses it
#[derive(Debug, Copy, Clone)]
struct CountingTestHasher(TreeSizeTestHasher);

impl NewMerkleHasherForTreeSize for CountingTestHasher {
    fn new_for_tree_size(num_leaves: u64) -> Self {
        Self(TreeSizeTestHasher::new_for_tree_size(num_leaves))
    }
}

impl MerkleHasher for CountingTestHasher {
    const OUT_LEN: usize = OUT_LEN;

    fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
        NUM_HASHES.fetch_add(1, Ordering::Relaxed);
        self.0.hash_pair(pair)
    }

    fn hash_single_leaf(&self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
        NUM_HASHES.fetch_add(1, Ordering::Relaxed);
        self.0.hash_single_leaf(leaf)
    }
}

/// The largest number of leaves in sparse tree tests
const SPARSE_MAX_N: u64 = u16::MAX as u64;

/// Leaves at `positions` with random contents
fn sparse_leaves(rng: &mut ChaCha8Rng, positions: &[u64]) -> Vec<(u64, [u8; OUT_LEN])> {
    positions
        .iter()
        .map(|&position| {
            let mut leaf = [0u8; OUT_LEN];
            rng.fill_bytes(&mut leaf);
            (position, leaf)
        })
        .collect()
}

/// Random strictly increasing positions in a tree with `num_leaves` leaves
fn random_positions(rng: &mut ChaCha8Rng, num_leaves: u64) -> Vec<u64> {
    // Different densities, from almost all zero leaves to almost no zero leaves
    let density = rng.next_u32() % 4;
    (0..num_leaves)
        .filter(|_| match density {
            0 => rng.next_u32().is_multiple_of(64),
            1 => rng.next_u32().is_multiple_of(8),
            2 => rng.next_u32().is_multiple_of(2),
            _ => !rng.next_u32().is_multiple_of(8),
        })
        .collect()
}

/// Compare the sparse root with the root of the same leaves with zeroes in all other positions
fn check_sparse_root(rng: &mut ChaCha8Rng, num_leaves: u64, positions: &[u64]) {
    let leaves = sparse_leaves(rng, positions);

    let mut dense_leaves = vec![[0u8; OUT_LEN]; num_leaves as usize];
    for &(position, leaf) in &leaves {
        dense_leaves[position as usize] = leaf;
    }
    let dense_root =
        TreeSizeUnbalanced::compute_root_only::<SPARSE_MAX_N, _, _>(dense_leaves.iter().copied());
    assert!(dense_root.is_some(), "num_leaves {num_leaves}");

    assert_eq!(
        TreeSizeUnbalanced::compute_root_only_sparse::<SPARSE_MAX_N, _, _>(num_leaves, leaves),
        dense_root,
        "num_leaves {num_leaves} positions {positions:?}"
    );
}

/// The sparse root is the same as the root of all leaves with zeroes in all other positions
#[test]
#[cfg_attr(miri, ignore)]
fn sparse_root_matches_dense_root() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    for num_leaves in 1..=300 {
        check_sparse_root(&mut rng, num_leaves, &[]);
        check_sparse_root(&mut rng, num_leaves, &(0..num_leaves).collect::<Vec<_>>());
        for position in 0..num_leaves {
            check_sparse_root(&mut rng, num_leaves, &[position]);
        }
        for _ in 0..8 {
            let positions = random_positions(&mut rng, num_leaves);
            check_sparse_root(&mut rng, num_leaves, &positions);
        }
    }

    for num_leaves in [1000, 4095, 4096, 4097, SPARSE_MAX_N] {
        check_sparse_root(&mut rng, num_leaves, &[]);
        check_sparse_root(&mut rng, num_leaves, &[0]);
        check_sparse_root(&mut rng, num_leaves, &[num_leaves - 1]);
        check_sparse_root(&mut rng, num_leaves, &[0, num_leaves / 2, num_leaves - 1]);
        for _ in 0..4 {
            let positions = random_positions(&mut rng, num_leaves);
            check_sparse_root(&mut rng, num_leaves, &positions);
        }
    }
}

/// Small trees, such that Miri checks unchecked indexing too
#[test]
fn sparse_root_matches_dense_root_small() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    for num_leaves in 1..=9 {
        check_sparse_root(&mut rng, num_leaves, &[]);
        check_sparse_root(&mut rng, num_leaves, &(0..num_leaves).collect::<Vec<_>>());
        for position in 0..num_leaves {
            check_sparse_root(&mut rng, num_leaves, &[position]);
        }
        let positions = random_positions(&mut rng, num_leaves);
        check_sparse_root(&mut rng, num_leaves, &positions);
    }
}

/// Indices must be strictly increasing and smaller than the number of leaves, which must be
/// between 1 and `MAX_N`
#[test]
fn sparse_root_rejects_invalid_indices() {
    let a = [1; OUT_LEN];
    let b = [2; OUT_LEN];
    let root = |num_leaves, leaves: Vec<(u64, [u8; OUT_LEN])>| {
        TreeSizeUnbalanced::compute_root_only_sparse::<8, _, _>(num_leaves, leaves)
    };

    assert!(root(8, vec![(1, a), (3, b)]).is_some());
    assert!(root(8, vec![(7, a)]).is_some());
    assert!(root(1, vec![]).is_some());

    // Decreasing indices
    assert_eq!(root(8, vec![(3, a), (1, b)]), None);
    // Repeated index
    assert_eq!(root(8, vec![(3, a), (3, b)]), None);
    // Index equal to or larger than the number of leaves
    assert_eq!(root(8, vec![(1, a), (8, b)]), None);
    assert_eq!(root(5, vec![(5, a)]), None);
    assert_eq!(root(5, vec![(u64::MAX, a)]), None);
    // No leaves
    assert_eq!(root(0, vec![]), None);
    assert_eq!(root(0, vec![(0, a)]), None);
    // More leaves than `MAX_N`
    assert_eq!(root(9, vec![]), None);
    assert_eq!(root(9, vec![(0, a)]), None);
    assert_eq!(root(u64::MAX, vec![]), None);
}

/// Runs of zero leaves are hashed as whole subtrees, so the number of hashes depends on the number
/// of non-zero leaves and the height of the tree rather than on the number of leaves
#[test]
fn sparse_root_hashes_zero_runs_as_subtrees() {
    const HEIGHT: u64 = SPARSE_MAX_N.next_power_of_two().ilog2() as u64;

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let num_hashes = |rng: &mut ChaCha8Rng, positions: &[u64]| {
        let leaves = sparse_leaves(rng, positions);
        NUM_HASHES.store(0, Ordering::Relaxed);
        let root = unbalanced::UnbalancedMerkleTree::<CountingTestHasher>::compute_root_only_sparse::<
            SPARSE_MAX_N,
            _,
            _,
        >(SPARSE_MAX_N, leaves);
        assert!(root.is_some());
        u64::from(NUM_HASHES.load(Ordering::Relaxed))
    };

    // At most `HEIGHT` merges above each non-zero leaf, `HEIGHT` roots of zero subtrees and
    // `HEIGHT` hashes of peaks, plus the hash of a single leaf
    assert!(num_hashes(&mut rng, &[]) <= 2 * HEIGHT + 1);

    let mut positions = iter::repeat_with(|| u64::from(rng.next_u32()) % SPARSE_MAX_N)
        .take(16)
        .collect::<Vec<_>>();
    positions.sort_unstable();
    positions.dedup();
    let num_positions = positions.len() as u64;
    assert!(num_hashes(&mut rng, &positions) <= (num_positions + 2) * HEIGHT + 1);
}
