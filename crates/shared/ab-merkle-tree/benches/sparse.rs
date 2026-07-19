#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(generic_const_args, min_generic_const_args)]

use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::sparse::{Leaf, PROOF_ELEMENTS, SparseMerkleTree};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::num::NonZeroU128;

fn criterion_benchmark(c: &mut Criterion) {
    // Intentional inlining prevention doesn't allow the compiler to prove lack of panics
    if cfg!(feature = "no-panic") {
        return;
    }

    // Fully materialized trees (all leaves occupied), cross-checked against the Balanced Merkle
    // Tree
    sparse_full::<8, 256>(c);
    sparse_full::<12, 4096>(c);
    sparse_full::<16, 65536>(c);

    // Sparse trees of intractable size, where the vast majority of leaves are empty
    sparse_skip::<32>(c);
    sparse_skip::<64>(c);
    sparse_skip::<128>(c);
}

/// Benchmarks a fully occupied tree of `2^BITS == N` leaves
fn sparse_full<const BITS: u8, const N: usize>(c: &mut Criterion) {
    assert_eq!(2usize.pow(u32::from(BITS)), N);

    // SAFETY: Data structure filled with zeroes is a valid invariant
    let mut input = unsafe { Box::<[[u8; 32]; N]>::new_zeroed().assume_init() };
    for (index, input) in input.iter_mut().enumerate() {
        *input = [(index % u8::MAX as usize + 1) as u8; 32];
    }

    c.bench_function(&format!("{BITS}/sparse/compute-root-only"), |b| {
        b.iter(|| {
            black_box(SparseMerkleTree::<BITS>::compute_root_only(
                black_box(&input).iter().map(|leaf| Leaf::Occupied { leaf }),
            ));
        });
    });

    // A fully occupied Sparse Merkle Tree has the same root and proofs as a Balanced Merkle Tree,
    // which is used here to conveniently generate proofs for the verification benchmark
    let mut instance = Box::new_uninit();
    let tree = &*BalancedMerkleTree::<N>::new_in(&mut instance, &input);
    let root = tree.root();

    let indices = (0..N).step_by(100).collect::<Vec<_>>();
    let proofs = tree
        .all_proofs()
        .enumerate()
        .filter(|(index, _proof)| indices.contains(index))
        .map(|(_index, proof)| {
            <[[u8; 32]; PROOF_ELEMENTS::<BITS>]>::try_from(proof.as_slice())
                .expect("Balanced Merkle Tree proof has exactly `BITS` elements; qed")
        })
        .collect::<Vec<_>>();

    c.bench_function(&format!("{BITS}/sparse/verify"), |b| {
        b.iter(|| {
            for (&index, proof) in indices.iter().zip(&proofs) {
                black_box(SparseMerkleTree::<BITS>::verify(
                    black_box(&root),
                    black_box(proof),
                    black_box(index as u128),
                    black_box(input[index]),
                ));
            }
        });
    });
}

/// Benchmarks a tree of intractable `2^BITS` size, where only a single leaf is occupied and the
/// rest are empty
fn sparse_skip<const BITS: u8>(c: &mut Criterion) {
    let leaf = [1u8; 32];
    // Number of empty leaves following the single occupied leaf
    let skip_count = if u32::from(BITS) < u128::BITS {
        NonZeroU128::new((1u128 << BITS) - 1).unwrap()
    } else {
        NonZeroU128::MAX
    };

    c.bench_function(&format!("{BITS}/sparse/compute-root-only-skip"), |b| {
        b.iter(|| {
            black_box(SparseMerkleTree::<BITS>::compute_root_only([
                black_box(Leaf::Occupied { leaf: &leaf }),
                black_box(Leaf::Empty { skip_count }),
            ]));
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
