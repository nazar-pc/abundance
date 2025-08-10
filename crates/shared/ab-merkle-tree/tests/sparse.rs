#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs)]

use ab_blake3::OUT_LEN;
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::hash_pair;
use ab_merkle_tree::sparse::{Leaf, SparseMerkleTree};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

const ZERO: [u8; OUT_LEN] = [0; OUT_LEN];

#[test]
fn smt_empty() {
    assert_eq!(SparseMerkleTree::<1>::compute_root_only([]).unwrap(), ZERO);
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([Leaf::Empty { skip_count: 1 }]).unwrap(),
        ZERO
    );
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([
            Leaf::Empty { skip_count: 1 },
            Leaf::Empty { skip_count: 0 },
            Leaf::Empty { skip_count: 1 },
        ])
        .unwrap(),
        ZERO
    );
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([Leaf::Empty { skip_count: 2 }]).unwrap(),
        ZERO
    );

    assert_eq!(SparseMerkleTree::<2>::compute_root_only([]).unwrap(), ZERO);
    assert_eq!(
        SparseMerkleTree::<2>::compute_root_only([Leaf::Empty { skip_count: 4 }]).unwrap(),
        ZERO
    );
}

#[test]
fn smt_too_many_leaves() {
    // 3 leaves is more than 2^1=2
    assert!(SparseMerkleTree::<1>::compute_root_only([ZERO; 3].iter().map(Leaf::from)).is_none());
    assert!(SparseMerkleTree::<1>::compute_root_only([Leaf::Empty { skip_count: 3 }]).is_none());
    assert!(
        SparseMerkleTree::<1>::compute_root_only(
            [ZERO; 2]
                .iter()
                .map(Leaf::from)
                .chain([Leaf::Empty { skip_count: 1 }])
        )
        .is_none()
    );
    // Even an empty skip at the end is not supported
    assert!(
        SparseMerkleTree::<1>::compute_root_only(
            [ZERO; 2]
                .iter()
                .map(Leaf::from)
                .chain([Leaf::Empty { skip_count: 0 }])
        )
        .is_none()
    );
}

#[test]
fn smt_32_full() {
    const N: usize = 32;
    const BITS: u8 = N.ilog2() as u8;

    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = [[0u8; OUT_LEN]; N];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    // Full tree (unlikely to exist in practice) is identical to the regular Balanced Merkle Tree
    assert_eq!(
        SparseMerkleTree::<BITS>::compute_root_only(
            leaves.iter().map(|leaf| Leaf::Occupied { leaf })
        )
        .unwrap(),
        BalancedMerkleTree::compute_root_only(&leaves)
    );
}

#[test]
fn smt_32_1_missing() {
    test_32(1);
}

#[test]
fn smt_32_2_missing() {
    test_32(2);
}

#[test]
fn smt_32_3_missing() {
    test_32(3);
}

#[test]
fn smt_32_4_missing() {
    test_32(4);
}

#[test]
fn smt_32_5_missing() {
    test_32(5);
}

#[test]
fn smt_128_bit() {
    const BITS: u8 = u128::BITS as u8;

    assert_eq!(
        SparseMerkleTree::<BITS>::compute_root_only([]).unwrap(),
        ZERO
    );
    assert_eq!(
        SparseMerkleTree::<BITS>::compute_root_only([
            Leaf::Empty {
                skip_count: u128::MAX
            },
            Leaf::Empty { skip_count: 1 }
        ])
        .unwrap(),
        ZERO
    );

    {
        let first_leaf = [1; OUT_LEN];
        let correct_root = {
            let mut root = first_leaf;

            for _ in 0..BITS {
                root = hash_pair(&root, &ZERO);
            }

            root
        };

        assert_eq!(
            SparseMerkleTree::<BITS>::compute_root_only([Leaf::Occupied { leaf: &first_leaf }])
                .unwrap(),
            correct_root
        );
        assert_eq!(
            SparseMerkleTree::<BITS>::compute_root_only([
                Leaf::Occupied { leaf: &first_leaf },
                Leaf::Empty {
                    skip_count: u128::MAX
                }
            ])
            .unwrap(),
            correct_root
        );

        // Even an empty skip at the end is not supported
        assert!(
            SparseMerkleTree::<BITS>::compute_root_only([
                Leaf::Occupied { leaf: &first_leaf },
                Leaf::Empty {
                    skip_count: u128::MAX
                },
                Leaf::Empty { skip_count: 0 }
            ])
            .is_none()
        );
    }
    {
        let last_leaf = [2; OUT_LEN];
        let correct_root = {
            let mut root = last_leaf;

            for _ in 0..BITS {
                root = hash_pair(&ZERO, &root);
            }

            root
        };

        assert_eq!(
            SparseMerkleTree::<BITS>::compute_root_only([
                Leaf::Empty {
                    skip_count: u128::MAX
                },
                Leaf::Occupied { leaf: &last_leaf }
            ])
            .unwrap(),
            correct_root
        );

        assert!(
            SparseMerkleTree::<BITS>::compute_root_only([
                Leaf::Empty {
                    skip_count: u128::MAX
                },
                Leaf::Empty { skip_count: 1 },
                Leaf::Occupied { leaf: &last_leaf }
            ])
            .is_none()
        );
    }
}

/// Inefficient, full of allocations, but very simple to understand implementation
fn naive_sparse_merkle_tree_root(leaves: &[[u8; OUT_LEN]]) -> [u8; OUT_LEN] {
    let mut level = leaves.to_vec();

    while level.len() > 1 {
        level = level
            .as_chunks()
            .0
            .iter()
            .map(|[left, right]| {
                if left == &ZERO && right == &ZERO {
                    ZERO
                } else {
                    hash_pair(left, right)
                }
            })
            .collect();
    }

    level.into_iter().next().unwrap()
}

fn test_32(total_skip_size: usize) {
    const N: usize = 32;
    const BITS: u8 = N.ilog2() as u8;

    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let leaves = {
        let mut leaves = [[0u8; OUT_LEN]; N];
        for hash in &mut leaves {
            rng.fill_bytes(hash);
        }
        leaves
    };

    // Try all permutations of the location of the full total skip size within a set of leaves
    for offset in 0..N - total_skip_size {
        let mut modified_leaves = leaves;
        modified_leaves[offset..][..total_skip_size].fill(ZERO);

        let correct_root = naive_sparse_merkle_tree_root(&modified_leaves);

        // Skip one leaf at a time
        assert_eq!(
            SparseMerkleTree::<BITS>::compute_root_only(modified_leaves.iter().map(|leaf| {
                if leaf == &ZERO {
                    Leaf::Empty { skip_count: 1 }
                } else {
                    Leaf::Occupied { leaf }
                }
            }))
            .unwrap(),
            correct_root,
            "offset={offset} total_skip_size={total_skip_size}"
        );

        // Skip all leaves at once
        assert_eq!(
            SparseMerkleTree::<BITS>::compute_root_only(
                modified_leaves[..offset]
                    .iter()
                    .map(Leaf::from)
                    .chain([Leaf::Empty {
                        skip_count: total_skip_size as u128
                    }])
                    // Throw an empty skip in the middle, shouldn't impact anything
                    .chain([Leaf::Empty { skip_count: 0 }])
                    .chain(
                        modified_leaves[offset + total_skip_size..]
                            .iter()
                            .map(Leaf::from)
                    )
            )
            .unwrap(),
            correct_root,
            "offset={offset} total_skip_size={total_skip_size}"
        );

        if let Some(stripped) =
            modified_leaves.strip_suffix(&modified_leaves[offset..][..total_skip_size])
        {
            // Do not specify empty leaves at the end
            assert_eq!(
                SparseMerkleTree::<BITS>::compute_root_only(stripped.iter().map(Leaf::from))
                    .unwrap(),
                correct_root,
                "offset={offset} total_skip_size={total_skip_size}"
            );
        }
    }
}
