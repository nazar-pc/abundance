use crate::test_hasher::{
    BalancedMerkleTree, OUT_LEN, ParallelTestHasher2, ParallelTestHasher4, ParallelTestHasher8,
    ParallelTestHasher16, ParallelTestHasher32, ParallelTestHasher64, SparseMerkleTree, hash_pair,
};
use ab_merkle_tree::sparse::Leaf;
use ab_merkle_tree::{MerkleHasher, sparse};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::num::NonZeroU128;

const ZERO: [u8; OUT_LEN] = [0; OUT_LEN];

#[test]
fn smt_empty() {
    assert_eq!(SparseMerkleTree::<1>::compute_root_only([]).unwrap(), ZERO);
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([Leaf::Empty {
            skip_count: NonZeroU128::new(1).unwrap()
        }])
        .unwrap(),
        ZERO
    );
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([
            Leaf::Empty {
                skip_count: NonZeroU128::new(1).unwrap()
            },
            Leaf::Empty {
                skip_count: NonZeroU128::new(1).unwrap()
            },
        ])
        .unwrap(),
        ZERO
    );
    assert_eq!(
        SparseMerkleTree::<1>::compute_root_only([Leaf::Empty {
            skip_count: NonZeroU128::new(2).unwrap()
        }])
        .unwrap(),
        ZERO
    );

    assert_eq!(SparseMerkleTree::<2>::compute_root_only([]).unwrap(), ZERO);
    assert_eq!(
        SparseMerkleTree::<2>::compute_root_only([Leaf::Empty {
            skip_count: NonZeroU128::new(4).unwrap()
        }])
        .unwrap(),
        ZERO
    );
}

#[test]
fn smt_too_many_leaves() {
    // 3 leaves is more than 2^1=2
    assert!(SparseMerkleTree::<1>::compute_root_only([ZERO; 3].iter().map(Leaf::from)).is_none());
    assert!(
        SparseMerkleTree::<1>::compute_root_only([Leaf::Empty {
            skip_count: NonZeroU128::new(3).unwrap()
        }])
        .is_none()
    );
    assert!(
        SparseMerkleTree::<1>::compute_root_only([ZERO; 2].iter().map(Leaf::from).chain([
            Leaf::Empty {
                skip_count: NonZeroU128::new(1).unwrap()
            }
        ]))
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
fn smt_occupied_owned() {
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

    let root_borrowed =
        SparseMerkleTree::<BITS>::compute_root_only(leaves.iter().map(Leaf::from)).unwrap();
    // `Leaf::OccupiedOwned` must produce an identical root to `Leaf::Occupied` for the same values
    let root_owned = SparseMerkleTree::<BITS>::compute_root_only(
        leaves
            .iter()
            .map(|leaf| Leaf::OccupiedOwned { leaf: *leaf }),
    )
    .unwrap();
    assert_eq!(root_borrowed, root_owned);

    // Mixing borrowed and owned occupied leaves together must still produce the same root as an
    // all-borrowed sequence of the same values
    let mixed_root = SparseMerkleTree::<BITS>::compute_root_only(leaves.iter().enumerate().map(
        |(index, leaf)| {
            if index.is_multiple_of(2) {
                Leaf::OccupiedOwned { leaf: *leaf }
            } else {
                Leaf::Occupied { leaf }
            }
        },
    ))
    .unwrap();
    assert_eq!(mixed_root, root_borrowed);
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
                skip_count: NonZeroU128::MAX
            },
            Leaf::Empty {
                skip_count: NonZeroU128::new(1).unwrap()
            }
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
                    skip_count: NonZeroU128::MAX
                }
            ])
            .unwrap(),
            correct_root
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
                    skip_count: NonZeroU128::MAX
                },
                Leaf::Occupied { leaf: &last_leaf }
            ])
            .unwrap(),
            correct_root
        );

        assert!(
            SparseMerkleTree::<BITS>::compute_root_only([
                Leaf::Empty {
                    skip_count: NonZeroU128::MAX
                },
                Leaf::Empty {
                    skip_count: NonZeroU128::new(1).unwrap()
                },
                Leaf::Occupied { leaf: &last_leaf }
            ])
            .is_none()
        );
    }
}

#[test]
fn smt_32_verify() {
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

    for total_skip_size in [0, 1, 2, 5, 16, 31] {
        for offset in 0..N - total_skip_size {
            let mut modified_leaves = leaves;
            modified_leaves[offset..][..total_skip_size].fill(ZERO);

            let root = naive_sparse_merkle_tree_root(&modified_leaves);

            // Proofs of both occupied and empty leaves must verify
            for (index, leaf) in modified_leaves.iter().enumerate() {
                let proof = naive_sparse_merkle_tree_proof(&modified_leaves, index);
                let proof = <[[u8; OUT_LEN]; BITS as usize]>::try_from(proof).unwrap();
                assert!(
                    SparseMerkleTree::<BITS>::verify(&root, &proof, index as u128, *leaf),
                    "offset={offset} total_skip_size={total_skip_size} index={index}"
                );
            }
        }
    }
}

/// Hashers that hash pairs in parallel buffer consecutive occupied leaves and hash them as
/// perfect subtrees, which must not change the root, especially around boundaries of the buffer
#[test]
#[cfg_attr(miri, ignore = "Takes too long")]
fn smt_buffered() {
    test_buffered::<ParallelTestHasher2>();
    test_buffered::<ParallelTestHasher4>();
    test_buffered::<ParallelTestHasher8>();
    test_buffered::<ParallelTestHasher16>();
    test_buffered::<ParallelTestHasher32>();
    test_buffered::<ParallelTestHasher64>();
}

/// Small trees, such that Miri checks buffering too
#[test]
fn smt_buffered_small() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    check_buffered::<4, ParallelTestHasher2>(&mut rng, &[(0, 16)], false);
    check_buffered::<5, ParallelTestHasher2>(&mut rng, &[(1, 17), (1, 13)], false);
    check_buffered::<5, ParallelTestHasher2>(&mut rng, &[(3, 20)], true);
    for offset in [0, 1, 15, 16, 17, u128::MAX - 50] {
        check_buffered::<128, ParallelTestHasher2>(&mut rng, &[(offset, 33), (1, 17)], true);
    }
}

fn test_buffered<H>()
where
    H: MerkleHasher + Default,
{
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    // Full trees, including those that are not larger than the buffer
    check_buffered::<1, H>(&mut rng, &[(0, 2)], false);
    check_buffered::<5, H>(&mut rng, &[(0, 32)], false);
    check_buffered::<7, H>(&mut rng, &[(0, 128)], false);
    check_buffered::<8, H>(&mut rng, &[(0, 256)], false);
    check_buffered::<10, H>(&mut rng, &[(0, 1024)], false);

    let middle = 1 << 64;
    for run in [
        1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 300, 1000,
    ] {
        for offset in [
            0,
            1,
            15,
            16,
            17,
            127,
            128,
            129,
            1000,
            middle - 1,
            middle,
            middle + 12345,
            (1 << 127) - 129,
            0x0123_4567_89ab_cdef_0123_4567_89ab_cdef,
        ] {
            check_buffered::<128, H>(&mut rng, &[(offset, run)], true);
            check_buffered::<128, H>(&mut rng, &[(offset, run)], false);
            // Runs separated by a single empty leaf
            check_buffered::<128, H>(&mut rng, &[(offset, run), (1, run), (1, 7)], true);
            // The second run starts at a multiple of the largest buffer
            let gap = 128 - (offset + run as u128) % 128;
            check_buffered::<128, H>(&mut rng, &[(offset, run), (gap, 130)], false);
        }

        // Runs that end with the last leaf
        let offset = 0u128.wrapping_sub(run as u128);
        check_buffered::<128, H>(&mut rng, &[(offset, run)], false);
        check_buffered::<128, H>(&mut rng, &[(5, 3), (offset - 8, run)], false);

        if run <= 256 {
            check_buffered::<8, H>(&mut rng, &[(256 - run as u128, run)], false);
            check_buffered::<8, H>(&mut rng, &[(0, run)], true);
        }
        if run < 256 {
            check_buffered::<8, H>(&mut rng, &[(1, run / 2), (1, run / 2)], true);
        }
    }

    // Random runs with random gaps
    for _ in 0..100 {
        let mut runs = Vec::new();
        let mut num_leaves = 0u128;
        for _ in 0..(rng.next_u32() % 8) {
            let gap = match rng.next_u32() % 4 {
                0 => 1,
                1 => u128::from(rng.next_u32() % 300),
                2 => u128::from(rng.next_u64()),
                _ => 128 - num_leaves % 128,
            };
            let run = (rng.next_u32() % 400) as usize;
            num_leaves += gap + run as u128;
            runs.push((gap, run));
        }
        let trailing_gap = rng.next_u32().is_multiple_of(2);
        check_buffered::<128, H>(&mut rng, &runs, trailing_gap);
    }
}

/// Check the root of a tree with runs of occupied leaves, each is a pair of the number of empty
/// leaves before the run and the number of occupied leaves in it. The rest of the leaves are
/// explicitly empty with `trailing_gap` and implicitly otherwise.
fn check_buffered<const BITS: u8, H>(
    rng: &mut ChaCha8Rng,
    runs: &[(u128, usize)],
    trailing_gap: bool,
) where
    H: MerkleHasher + Default,
{
    let parallel_pairs = H::PARALLEL_PAIRS;
    let mut leaves = Vec::new();
    let mut num_leaves = 0u128;
    for &(gap, run) in runs {
        num_leaves += gap;
        for _ in 0..run {
            let mut leaf = [0u8; H::OUT_LEN];
            rng.fill_bytes(&mut leaf);
            leaves.push((num_leaves, leaf));
            // Wraps after the last leaf of a tree with `2^128` leaves
            num_leaves = num_leaves.wrapping_add(1);
        }
    }

    let mut tree_leaves = Vec::new();
    let mut occupied_leaves = leaves.iter();
    for (index, &(gap, run)) in runs.iter().enumerate() {
        if let Some(skip_count) = NonZeroU128::new(gap) {
            tree_leaves.push(Leaf::Empty { skip_count });
        }
        for (_, leaf) in occupied_leaves.by_ref().take(run) {
            // Both kinds of occupied leaves must be handled the same way
            if index.is_multiple_of(2) {
                tree_leaves.push(Leaf::Occupied { leaf });
            } else {
                tree_leaves.push(Leaf::OccupiedOwned { leaf: *leaf });
            }
        }
    }
    let num_trailing_empty = if u32::from(BITS) == u128::BITS {
        0u128.wrapping_sub(num_leaves)
    } else {
        (1 << BITS) - num_leaves
    };
    if trailing_gap && let Some(skip_count) = NonZeroU128::new(num_trailing_empty) {
        tree_leaves.push(Leaf::Empty { skip_count });
    }

    assert_eq!(
        sparse::SparseMerkleTree::<BITS, H>::compute_root_only(tree_leaves)
            .unwrap()
            .as_slice(),
        naive_subtree_root::<H>(0, u32::from(BITS), &leaves).as_slice(),
        "PARALLEL_PAIRS {parallel_pairs} BITS {BITS} runs {runs:?} trailing_gap {trailing_gap}"
    );
}

/// Root of a subtree of `2^height` leaves that starts at leaf `first_leaf` by definition, `leaves`
/// are occupied leaves of the subtree with their indices, all other leaves are empty
fn naive_subtree_root<H>(
    first_leaf: u128,
    height: u32,
    leaves: &[(u128, [u8; H::OUT_LEN])],
) -> [u8; H::OUT_LEN]
where
    H: MerkleHasher + Default,
{
    let zero = [0u8; H::OUT_LEN];
    if leaves.is_empty() {
        return zero;
    }
    if height == 0 {
        return leaves[0].1;
    }

    let middle = first_leaf + (1 << (height - 1));
    let (left, right) = leaves.split_at(leaves.partition_point(|&(index, _)| index < middle));
    let left = naive_subtree_root::<H>(first_leaf, height - 1, left);
    let right = naive_subtree_root::<H>(middle, height - 1, right);
    if left == zero && right == zero {
        zero
    } else {
        H::default().hash_pair(&[left, right])
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

/// Inefficient, full of allocations, but very simple to understand implementation
fn naive_sparse_merkle_tree_proof(
    leaves: &[[u8; OUT_LEN]],
    mut index: usize,
) -> Vec<[u8; OUT_LEN]> {
    let mut level = leaves.to_vec();
    let mut proof = Vec::new();

    while level.len() > 1 {
        proof.push(level[index ^ 1]);
        index /= 2;
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

    proof
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
                    Leaf::Empty {
                        skip_count: NonZeroU128::new(1).unwrap(),
                    }
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
                        skip_count: NonZeroU128::new(total_skip_size as u128).unwrap(),
                    }])
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
