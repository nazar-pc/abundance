#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_merkle_tree::mmr::MerkleMountainRange;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::mem::MaybeUninit;

const U64_TO_USIZE<const N: u64>: usize = N as usize;

fn criterion_benchmark(c: &mut Criterion) {
    // Intentional inlining prevention doesn't allow the compiler to prove lack of panics
    if cfg!(feature = "no-panic") {
        return;
    }

    mmr::<2>(c);
    mmr::<4>(c);
    mmr::<256>(c);
    mmr::<32768>(c);
    mmr::<65536>(c);
}

fn mmr<const MAX_N: u64>(c: &mut Criterion) {
    // SAFETY: Data structure filled with zeroes is a valid invariant
    let mut input = unsafe { Box::<[[u8; 32]; U64_TO_USIZE::<MAX_N>]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    // Aggregate all leaves in bulk and compute the root
    c.bench_function(&format!("{MAX_N}/mmr/add-leaves-root"), |b| {
        b.iter(|| {
            let mut mmr = MerkleMountainRange::<MAX_N>::new();
            black_box(mmr.add_leaves(black_box(input.iter().copied())));
            black_box(mmr.root());
        });
    });

    // Aggregate all leaves incrementally and compute the root
    c.bench_function(&format!("{MAX_N}/mmr/add-leaf-root"), |b| {
        b.iter(|| {
            let mut mmr = MerkleMountainRange::<MAX_N>::new();
            for leaf in black_box(&input) {
                black_box(mmr.add_leaf(black_box(leaf)));
            }
            black_box(mmr.root());
        });
    });

    // Aggregate all leaves incrementally, generating an inclusion proof for each of them
    c.bench_function(&format!("{MAX_N}/mmr/add-leaf-and-compute-proof"), |b| {
        b.iter(|| {
            let mut mmr = MerkleMountainRange::<MAX_N>::new();
            let mut proof = [MaybeUninit::uninit(); _];

            for leaf in black_box(&input) {
                black_box(mmr.add_leaf_and_compute_proof_in(black_box(leaf), &mut proof));
            }
        });
    });

    // Collect a root and proof snapshot for a subset of leaf indices to benchmark verification
    let indices = (0..input.len()).step_by(100).collect::<Vec<_>>();
    let snapshots = {
        let mut mmr = MerkleMountainRange::<MAX_N>::new();
        let mut proof = Box::new([MaybeUninit::uninit(); _]);
        let mut snapshots = Vec::new();

        for (index, leaf) in input.iter().enumerate() {
            let (root, proof) = mmr.add_leaf_and_compute_proof_in(leaf, &mut proof).unwrap();
            if indices.contains(&index) {
                snapshots.push((root, proof.to_vec(), index as u64, *leaf, index as u64 + 1));
            }
        }

        snapshots
    };

    c.bench_function(&format!("{MAX_N}/mmr/verify"), |b| {
        b.iter(|| {
            for (root, proof, leaf_index, leaf, num_leaves) in &snapshots {
                black_box(MerkleMountainRange::<MAX_N>::verify(
                    black_box(root),
                    black_box(proof),
                    black_box(*leaf_index),
                    black_box(*leaf),
                    black_box(*num_leaves),
                ));
            }
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
