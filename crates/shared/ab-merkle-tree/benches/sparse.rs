#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(gca_const_items, gca_macroless_args, gca_min_const_items)]

use ab_merkle_tree::blake3::{BalancedMerkleTree, SparseMerkleTree};
use ab_merkle_tree::sparse::{Leaf, PROOF_ELEMENTS};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::iter;
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
    sparse_run(c);
    sparse_scattered(c, "isolated-16", 16, 0);
    sparse_scattered(c, "isolated-1000", 1000, 0);
    sparse_scattered(c, "mixed", 100, 1000);
    sparse_scattered(c, "mixed-16", 16, 1000);
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

/// Benchmarks a tree of intractable `2^128` size with a run of consecutive occupied leaves at an
/// offset that is not aligned to a power of two, the rest of the leaves are empty
fn sparse_run(c: &mut Criterion) {
    let input = (0..1000)
        .map(|index| [(index % u8::MAX as usize + 1) as u8; 32])
        .collect::<Vec<_>>();
    let offset = NonZeroU128::new(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef).unwrap();

    c.bench_function("128/sparse/compute-root-only-run", |b| {
        b.iter(|| {
            black_box(SparseMerkleTree::<128>::compute_root_only(
                [Leaf::Empty { skip_count: offset }]
                    .into_iter()
                    .chain(black_box(&input).iter().map(|leaf| Leaf::Occupied { leaf })),
            ));
        });
    });
}

/// Benchmarks a tree of intractable `2^128` size with `num_isolated` occupied leaves at random
/// positions and a run of `run` consecutive occupied leaves at an offset that is not aligned to a
/// power of two, the rest of the leaves are empty
fn sparse_scattered(c: &mut Criterion, name: &str, num_isolated: usize, run: u128) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let offset = 0x0123_4567_89ab_cdef_0123_4567_89ab_cdef;
    let mut indices =
        iter::repeat_with(|| (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64()))
            .take(num_isolated)
            .chain(offset..offset + run)
            .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();

    // Each occupied leaf with the number of empty leaves before it
    let mut next_index = 0;
    let input = indices
        .iter()
        .map(|&index| {
            let mut leaf = [0; 32];
            rng.fill_bytes(&mut leaf);
            let gap = NonZeroU128::new(index - next_index);
            next_index = index + 1;
            (gap, leaf)
        })
        .collect::<Vec<_>>();

    c.bench_function(&format!("128/sparse/compute-root-only-{name}"), |b| {
        b.iter(|| {
            black_box(SparseMerkleTree::<128>::compute_root_only(
                black_box(&input).iter().flat_map(|(gap, leaf)| {
                    gap.map(|skip_count| Leaf::Empty { skip_count })
                        .into_iter()
                        .chain([Leaf::Occupied { leaf }])
                }),
            ));
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
