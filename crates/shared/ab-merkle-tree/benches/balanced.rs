#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(generic_const_args, min_generic_const_args)]

use ab_merkle_tree::balanced::BalancedMerkleTree;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    // Intentional inlining prevention doesn't allow the compiler to prove lack of panics
    if cfg!(feature = "no-panic") {
        return;
    }

    balanced::<2>(c);
    balanced::<4>(c);
    balanced::<256>(c);
    balanced::<32768>(c);
    balanced::<65536>(c);
}

fn balanced<const N: usize>(c: &mut Criterion) {
    // SAFETY: Data structure filled with zeroes is a valid invariant
    let mut input = unsafe { Box::<[[u8; 32]; N]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    let mut instance = Box::new_uninit();

    c.bench_function(&format!("{N}/balanced/new"), |b| {
        b.iter(|| {
            BalancedMerkleTree::new_in(black_box(&mut instance), black_box(&input));
        });
    });
    c.bench_function(&format!("{N}/balanced/compute-root-only"), |b| {
        b.iter(|| {
            black_box(BalancedMerkleTree::compute_root_only(black_box(&input)));
        });
    });

    let tree = &*BalancedMerkleTree::<N>::new_in(black_box(&mut instance), black_box(&input));

    c.bench_function(&format!("{N}/balanced/all-proofs"), |b| {
        b.iter(|| {
            black_box(black_box(black_box(tree).all_proofs()).count());
        });
    });

    let root = tree.root();
    let all_proofs = tree.all_proofs().collect::<Vec<_>>();

    c.bench_function(&format!("{N}/balanced/verify"), |b| {
        b.iter(|| {
            for (index, proof) in all_proofs.iter().enumerate() {
                black_box(BalancedMerkleTree::<N>::verify(
                    black_box(&root),
                    black_box(proof),
                    black_box(index),
                    black_box(input[index]),
                ));
            }
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
