#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(generic_const_exprs, maybe_uninit_slice, new_zeroed_alloc)]

use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use ab_merkle_tree::unbalanced_hashed::UnbalancedHashedMerkleTree;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::mem::MaybeUninit;

fn criterion_benchmark(c: &mut Criterion) {
    // Intentional inlining prevention doesn't allow compiler to prove lack of panics
    if cfg!(feature = "no-panic") {
        return;
    }

    balanced::<1>(c);
    balanced::<2>(c);
    balanced::<4>(c);
    balanced::<256>(c);
    balanced::<32768>(c);
    balanced::<65536>(c);

    unbalanced::<1, 1>(c);
    unbalanced::<2, 2>(c);
    unbalanced::<4, 4>(c);
    unbalanced::<256, 256>(c);
    unbalanced::<32768, 32768>(c);
    unbalanced::<65536, 65536>(c);

    // TODO: MMR benches
}

fn balanced<const N: usize>(c: &mut Criterion)
where
    [(); N - 1]:,
    [(); N.ilog2() as usize + 1]:,
{
    let mut input = unsafe { Box::<[[u8; 32]; N]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    let mut instance = Box::new_uninit();

    c.bench_function(&format!("{N}/balanced/new"), |b| {
        b.iter(|| {
            BalancedHashedMerkleTree::new_in(black_box(&mut instance), black_box(&input));
        })
    });
    c.bench_function(&format!("{N}/balanced/compute-root-only"), |b| {
        b.iter(|| {
            black_box(BalancedHashedMerkleTree::compute_root_only(black_box(
                &input,
            )));
        })
    });

    let tree = &*BalancedHashedMerkleTree::new_in(black_box(&mut instance), black_box(&input));

    c.bench_function(&format!("{N}/balanced/all-proofs"), |b| {
        b.iter(|| {
            black_box(black_box(black_box(tree).all_proofs()).count());
        })
    });

    let root = tree.root();
    let all_proofs = tree.all_proofs().collect::<Vec<_>>();

    c.bench_function(&format!("{N}/balanced/verify"), |b| {
        b.iter(|| {
            for (index, proof) in all_proofs.iter().enumerate() {
                black_box(BalancedHashedMerkleTree::<N>::verify(
                    black_box(&root),
                    black_box(proof),
                    black_box(index),
                    black_box(input[index]),
                ));
            }
        })
    });
}

fn unbalanced<const MAX_N: usize, const MAX_N_U64: u64>(c: &mut Criterion)
where
    [(); MAX_N_U64.ilog2() as usize + 1]:,
{
    let mut input = unsafe { Box::<[[u8; 32]; MAX_N]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    c.bench_function(&format!("{MAX_N}/unbalanced/compute-root-only"), |b| {
        b.iter(|| {
            black_box(UnbalancedHashedMerkleTree::compute_root_only(black_box(
                input.iter().copied(),
            )));
        })
    });

    {
        let indices = (0..input.len()).step_by(100).collect::<Vec<_>>();

        c.bench_function(&format!("{MAX_N}/unbalanced/compute-root-and-proof"), |b| {
            b.iter(|| {
                let mut proof = [MaybeUninit::uninit(); _];

                for &i in &indices {
                    black_box(UnbalancedHashedMerkleTree::compute_root_and_proof_in(
                        black_box(input.iter().copied()),
                        black_box(i),
                        black_box(&mut proof),
                    ));
                }
            })
        });

        let root = UnbalancedHashedMerkleTree::compute_root_only(input.iter().copied()).unwrap();
        let mut proofs = Vec::new();

        for &i in &indices {
            let mut proof = Box::new([MaybeUninit::uninit(); _]);

            let proof = UnbalancedHashedMerkleTree::compute_root_and_proof_in(
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
                    black_box(UnbalancedHashedMerkleTree::verify(
                        black_box(&root),
                        black_box(proof),
                        black_box(index as u64),
                        black_box(input[index]),
                        black_box(MAX_N_U64),
                    ));
                }
            })
        });
    }

    if MAX_N > 1 {
        let reduced_n = (MAX_N * 2 / 3).max(1);
        let input = &input[..reduced_n];

        c.bench_function(
            &format!("{reduced_n}({MAX_N})/unbalanced/compute-root-only"),
            |b| {
                b.iter(|| {
                    black_box(UnbalancedHashedMerkleTree::compute_root_only::<
                        MAX_N_U64,
                        _,
                        _,
                    >(black_box(input.iter().copied())));
                })
            },
        );

        c.bench_function(
            &format!("{reduced_n}({MAX_N})/unbalanced/compute-root-and-proof"),
            |b| {
                b.iter(|| {
                    let mut proof = [MaybeUninit::uninit(); _];

                    for i in (0..input.len()).step_by(100) {
                        black_box(UnbalancedHashedMerkleTree::compute_root_and_proof_in(
                            black_box(input.iter().copied()),
                            black_box(i),
                            black_box(&mut proof),
                        ));
                    }
                })
            },
        );
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
