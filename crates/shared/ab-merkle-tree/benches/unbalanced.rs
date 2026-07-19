#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(generic_const_args, generic_const_items, min_generic_const_args)]

use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::mem::MaybeUninit;

const U64_TO_USIZE<const N: u64>: usize = N as usize;

fn criterion_benchmark(c: &mut Criterion) {
    // Intentional inlining prevention doesn't allow the compiler to prove lack of panics
    if cfg!(feature = "no-panic") {
        return;
    }

    unbalanced::<1>(c);
    unbalanced::<2>(c);
    unbalanced::<4>(c);
    unbalanced::<256>(c);
    unbalanced::<32768>(c);
    unbalanced::<65536>(c);
}

fn unbalanced<const MAX_N: u64>(c: &mut Criterion) {
    // SAFETY: Data structure filled with zeroes is a valid invariant
    let mut input = unsafe { Box::<[[u8; 32]; U64_TO_USIZE::<MAX_N>]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    c.bench_function(&format!("{MAX_N}/unbalanced/compute-root-only"), |b| {
        b.iter(|| {
            black_box(UnbalancedMerkleTree::compute_root_only::<MAX_N, _, _>(
                black_box(input.iter().copied()),
            ));
        });
    });

    {
        let indices = (0..input.len()).step_by(100).collect::<Vec<_>>();

        c.bench_function(&format!("{MAX_N}/unbalanced/compute-root-and-proof"), |b| {
            b.iter(|| {
                let mut proof = [MaybeUninit::uninit(); _];

                for &i in &indices {
                    black_box(
                        UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
                            black_box(input.iter().copied()),
                            black_box(i),
                            black_box(&mut proof),
                        ),
                    );
                }
            });
        });

        let root =
            UnbalancedMerkleTree::compute_root_only::<MAX_N, _, _>(input.iter().copied()).unwrap();
        let mut proofs = Vec::new();

        for &i in &indices {
            let mut proof = Box::new([MaybeUninit::uninit(); _]);

            let proof = UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
                input.iter().copied(),
                i,
                &mut proof,
            )
            .unwrap()
            .1
            .to_vec();

            proofs.push(proof);
        }

        c.bench_function(&format!("{MAX_N}/unbalanced/verify"), |b| {
            b.iter(|| {
                for (&index, proof) in indices.iter().zip(&proofs) {
                    black_box(UnbalancedMerkleTree::verify(
                        black_box(&root),
                        black_box(proof),
                        black_box(index as u64),
                        black_box(input[index]),
                        black_box(MAX_N),
                    ));
                }
            });
        });
    }

    if MAX_N > 1 {
        let reduced_n = (MAX_N * 2 / 3).max(1) as usize;
        let input = &input[..reduced_n];

        c.bench_function(
            &format!("{reduced_n}({MAX_N})/unbalanced/compute-root-only"),
            |b| {
                b.iter(|| {
                    black_box(UnbalancedMerkleTree::compute_root_only::<MAX_N, _, _>(
                        black_box(input.iter().copied()),
                    ));
                });
            },
        );

        c.bench_function(
            &format!("{reduced_n}({MAX_N})/unbalanced/compute-root-and-proof"),
            |b| {
                b.iter(|| {
                    let mut proof = [MaybeUninit::uninit(); _];

                    for i in (0..input.len()).step_by(100) {
                        black_box(
                            UnbalancedMerkleTree::compute_root_and_proof_in::<MAX_N, _, _>(
                                black_box(input.iter().copied()),
                                black_box(i),
                                black_box(&mut proof),
                            ),
                        );
                    }
                });
            },
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
