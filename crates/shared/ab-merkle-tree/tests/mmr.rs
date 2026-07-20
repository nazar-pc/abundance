#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(generic_const_args, min_generic_const_args)]

//! Tests for [`MerkleMountainRange`]-specific behavior that is not covered by `balanced.rs` and
//! `unbalanced.rs` (which focus on cross-checking the root and proofs against the other Merkle Tree
//! implementations).

use ab_blake3::OUT_LEN;
use ab_merkle_tree::mmr::{MerkleMountainRange, MerkleMountainRangeBytes, MmrPeaks};
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::mem::MaybeUninit;

fn random_leaves<const N: usize>(rng: &mut ChaCha8Rng) -> [[u8; OUT_LEN]; N] {
    let mut leaves = [[0u8; OUT_LEN]; N];
    for hash in &mut leaves {
        rng.fill_bytes(hash);
    }
    leaves
}

#[test]
fn mmr_empty() {
    let mmr = MerkleMountainRange::<8>::new();

    // An empty Merkle Mountain Range has no root
    assert_eq!(mmr.num_leaves(), 0);
    assert!(mmr.root().is_none());
    // `Default` is identical to `new()`
    assert_eq!(
        mmr.num_leaves(),
        MerkleMountainRange::<8>::default().num_leaves()
    );

    // Peaks of an empty instance round-trip back to an empty instance
    let peaks = mmr.peaks();
    assert_eq!(peaks.num_leaves, 0);
    assert_eq!(peaks.num_peaks(), 0);
    let restored = MerkleMountainRange::<8>::from_peaks(&peaks).unwrap();
    assert_eq!(restored.num_leaves(), 0);
    assert!(restored.root().is_none());
}

#[test]
fn mmr_too_many_leaves() {
    const MAX_N: u64 = 3;

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let leaves = random_leaves::<{ MAX_N as usize + 1 }>(&mut rng);

    let mut mmr = MerkleMountainRange::<MAX_N>::new();
    for leaf in leaves.iter().take(MAX_N as usize) {
        assert!(mmr.add_leaf(leaf));
    }
    assert_eq!(mmr.num_leaves(), MAX_N);
    let root = mmr.root().unwrap();

    // Adding one leaf beyond the maximum must fail and leave the instance untouched
    let extra_leaf = &leaves[MAX_N as usize];
    {
        let mut mmr = mmr;
        assert!(!mmr.add_leaf(extra_leaf));
        assert_eq!(mmr.num_leaves(), MAX_N);
        assert_eq!(mmr.root().unwrap(), root);
    }
    {
        let mut mmr = mmr;
        assert!(!mmr.add_leaves([extra_leaf].into_iter().copied()));
        assert_eq!(mmr.num_leaves(), MAX_N);
        assert_eq!(mmr.root().unwrap(), root);
    }
    {
        let mut mmr = mmr;
        let proof_buffer = &mut [MaybeUninit::uninit(); _];
        assert!(
            mmr.add_leaf_and_compute_proof_in(extra_leaf, proof_buffer)
                .is_none()
        );
        assert_eq!(mmr.num_leaves(), MAX_N);
        assert_eq!(mmr.root().unwrap(), root);
    }
    #[cfg(feature = "alloc")]
    {
        let mut mmr = mmr;
        assert!(mmr.add_leaf_and_compute_proof(extra_leaf).is_none());
        assert_eq!(mmr.num_leaves(), MAX_N);
        assert_eq!(mmr.root().unwrap(), root);
    }

    // Bulk insertion stops as soon as capacity is exhausted
    let mut mmr = MerkleMountainRange::<MAX_N>::new();
    assert!(!mmr.add_leaves(leaves.iter().copied()));
    assert_eq!(mmr.num_leaves(), MAX_N);
    assert_eq!(mmr.root().unwrap(), root);
}

#[test]
fn mmr_bytes_round_trip() {
    const MAX_N: u64 = 8;

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let leaves = random_leaves::<{ MAX_N as usize }>(&mut rng);

    // Round-trip the state for every possible number of leaves
    for num_leaves in 0..=leaves.len() {
        let mut mmr = MerkleMountainRange::<MAX_N>::new();
        assert!(mmr.add_leaves(leaves.iter().copied().take(num_leaves)));

        // A default byte representation corresponds to an empty instance
        let bytes = *mmr.as_bytes();
        // `Deref` exposes the underlying byte array
        assert_eq!(
            bytes.len(),
            MerkleMountainRangeBytes::<MAX_N>::default().len()
        );

        // Byte array conversions round-trip
        let array: [u8; _] = bytes.into();
        let bytes_from_array = MerkleMountainRangeBytes::<MAX_N>::from(array);
        assert_eq!(bytes.as_slice(), bytes_from_array.as_slice());

        // SAFETY: Bytes were just produced by `as_bytes()`
        let restored = unsafe { MerkleMountainRange::<MAX_N>::from_bytes(&bytes_from_array) };
        assert_eq!(restored.num_leaves(), mmr.num_leaves());
        assert_eq!(restored.root(), mmr.root());
        assert_eq!(restored.peaks(), mmr.peaks());
    }

    // `DerefMut` allows mutating the underlying byte array
    let mut bytes = MerkleMountainRangeBytes::<MAX_N>::default();
    bytes[0] = 1;
    assert_eq!(bytes[0], 1);
}

#[test]
#[cfg_attr(miri, ignore)]
fn mmr_bytes_round_trip_large() {
    const MAX_N: u64 = 65536;

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    // SAFETY: Data structure filled with zeroes is a valid invariant
    let mut leaves = unsafe { Box::<[[u8; OUT_LEN]; MAX_N as usize]>::new_zeroed().assume_init() };
    for leaf in &mut leaves {
        rng.fill_bytes(leaf);
    }

    // Counts chosen to produce non-trivial peak sets (several simultaneously active levels), a
    // single peak (power of two) and the fully saturated case, at a scale where the `#[repr(C)]`
    // byte layout spans many more stack levels than the `MAX_N = 8` case above
    for &num_leaves in &[0usize, 1, 2, 3, 100, 12_345, 65_535, 65_536] {
        let mut mmr = MerkleMountainRange::<MAX_N>::new();
        assert!(mmr.add_leaves(leaves.iter().copied().take(num_leaves)));

        let bytes = *mmr.as_bytes();
        // SAFETY: Bytes were just produced by `as_bytes()`
        let restored = unsafe { MerkleMountainRange::<MAX_N>::from_bytes(&bytes) };

        assert_eq!(
            restored.num_leaves(),
            mmr.num_leaves(),
            "num_leaves {num_leaves}"
        );
        assert_eq!(restored.root(), mmr.root(), "num_leaves {num_leaves}");
        assert_eq!(restored.peaks(), mmr.peaks(), "num_leaves {num_leaves}");
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn mmr_peak_merging_boundaries() {
    const MAX_N: u64 = 128;
    // Counts landing exactly on power-of-two boundaries (where a single insertion merges the
    // maximum number of peaks), immediately before/after them, values with many bits set (forcing
    // a long cascade of merges on the very next insertion), and full capacity saturation
    const BOUNDARY_COUNTS: &[usize] = &[
        1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 85, 100, 101, 127, 128,
    ];

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let leaves = random_leaves::<{ MAX_N as usize }>(&mut rng);

    let mmr_proof_buffer = &mut [MaybeUninit::uninit(); _];
    let unbalanced_proof_buffer = &mut [MaybeUninit::uninit(); _];

    for &num_leaves in BOUNDARY_COUNTS {
        let prefix = &leaves[..num_leaves];

        let mut mmr = MerkleMountainRange::<MAX_N>::new();

        for (leaf_index, leaf) in prefix.iter().enumerate() {
            let (mmr_root, mmr_proof) = mmr
                .add_leaf_and_compute_proof_in(leaf, mmr_proof_buffer)
                .unwrap();

            // After each insertion the MMR represents a tree of exactly `leaf_index + 1` leaves,
            // so the reference must be computed over that same sub-prefix, not the full one
            let (expected_root, expected_proof) =
                UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
                    prefix[..=leaf_index].iter().copied(),
                    leaf_index,
                    unbalanced_proof_buffer,
                )
                .unwrap();

            assert_eq!(
                mmr_root, expected_root,
                "num_leaves {num_leaves} leaf_index {leaf_index}"
            );
            assert_eq!(
                mmr_proof, expected_proof,
                "num_leaves {num_leaves} leaf_index {leaf_index}"
            );
            assert!(
                MerkleMountainRange::<MAX_N>::verify(
                    &mmr_root,
                    mmr_proof,
                    leaf_index as u64,
                    *leaf,
                    leaf_index as u64 + 1
                ),
                "num_leaves {num_leaves} leaf_index {leaf_index}"
            );
        }

        assert_eq!(
            mmr.num_leaves(),
            num_leaves as u64,
            "num_leaves {num_leaves}"
        );
        assert_eq!(
            mmr.root().unwrap(),
            UnbalancedMerkleTree::compute_root_only::<MAX_N, _, _>(prefix.iter().copied()).unwrap(),
            "num_leaves {num_leaves}"
        );
    }
}

#[test]
fn mmr_peaks_round_trip() {
    const MAX_N: u64 = 32;

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let leaves = random_leaves::<{ MAX_N as usize }>(&mut rng);

    for num_leaves in 0..=leaves.len() {
        let mut mmr = MerkleMountainRange::<MAX_N>::new();
        assert!(mmr.add_leaves(leaves.iter().copied().take(num_leaves)));

        let peaks = mmr.peaks();
        assert_eq!(peaks.num_leaves, num_leaves as u64);
        // The number of occupied peaks equals the number of set bits in the number of leaves
        assert_eq!(
            u32::from(peaks.num_peaks()),
            (num_leaves as u64).count_ones(),
            "num_leaves {num_leaves}"
        );

        let restored = MerkleMountainRange::<MAX_N>::from_peaks(&peaks).unwrap();
        assert_eq!(
            restored.num_leaves(),
            mmr.num_leaves(),
            "num_leaves {num_leaves}"
        );
        assert_eq!(restored.root(), mmr.root(), "num_leaves {num_leaves}");
        assert_eq!(restored.peaks(), peaks, "num_leaves {num_leaves}");
    }
}

#[test]
fn mmr_from_peaks_invalid() {
    const MAX_N: u64 = 4;

    // A number of leaves that doesn't fit into `MAX_N` references peaks/stack levels that don't
    // exist, hence must be rejected
    let peaks = MmrPeaks::<MAX_N> {
        num_leaves: 8,
        peaks: [[0u8; OUT_LEN]; _],
    };
    assert!(MerkleMountainRange::<MAX_N>::from_peaks(&peaks).is_none());
}
