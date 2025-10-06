#![feature(const_trait_impl)]

#[cfg(feature = "alloc")]
use ab_core_primitives::pos::PosSeed;
use ab_core_primitives::sectors::SBucket;
use ab_proof_of_space::Table;
#[cfg(feature = "alloc")]
use ab_proof_of_space::TableGenerator;
#[cfg(feature = "alloc")]
use criterion::Throughput;
use criterion::{Criterion, criterion_group, criterion_main};
#[cfg(feature = "parallel")]
use rayon::ThreadPoolBuilder;
#[cfg(feature = "alloc")]
use std::hint::black_box;

#[cfg(not(feature = "alloc"))]
#[expect(
    clippy::extra_unused_type_parameters,
    reason = "Needs to match the normal version of the function"
)]
fn pos_bench<PosTable>(_c: &mut Criterion, _name: &'static str, _s_bucket_with_proof: SBucket)
where
    PosTable: Table,
{
    panic!(
        "`alloc` feature needs to be enabled to run benchmarks (`parallel` for benchmarking \
        parallel version)"
    )
}

#[cfg(feature = "alloc")]
fn pos_bench<PosTable>(c: &mut Criterion, name: &'static str, s_bucket_with_proof: SBucket)
where
    PosTable: Table,
{
    let seed = PosSeed::from([
        35, 2, 52, 4, 51, 55, 23, 84, 91, 10, 111, 12, 13, 222, 151, 16, 228, 211, 254, 45, 92,
        198, 204, 10, 9, 10, 11, 129, 139, 171, 15, 23,
    ]);

    #[cfg(feature = "parallel")]
    {
        // Repeated initialization is not supported, we just ignore errors here because of it
        let _ = ThreadPoolBuilder::new()
            // Change number of threads if necessary
            // .num_threads(4)
            .build_global();
    }

    let mut group = c.benchmark_group(name);

    let generator = PosTable::generator();
    group.throughput(Throughput::Elements(1));
    group.bench_function("proofs/single/1x", |b| {
        b.iter(|| {
            generator.create_proofs(black_box(&seed));
        });
    });

    #[cfg(feature = "parallel")]
    {
        {
            group.throughput(Throughput::Elements(2));
            group.bench_function("proofs/single/2x", |b| {
                b.iter(|| {
                    rayon::scope(|scope| {
                        for _ in 0..2 {
                            scope.spawn(|_scope| {
                                generator.create_proofs(black_box(&seed));
                            });
                        }
                    });
                });
            });
        }

        {
            group.throughput(Throughput::Elements(4));
            group.bench_function("proofs/single/4x", |b| {
                b.iter(|| {
                    rayon::scope(|scope| {
                        for _ in 0..4 {
                            scope.spawn(|_scope| {
                                generator.create_proofs(black_box(&seed));
                            });
                        }
                    });
                });
            });
        }

        {
            group.throughput(Throughput::Elements(8));
            group.bench_function("proofs/single/8x", |b| {
                b.iter(|| {
                    rayon::scope(|scope| {
                        for _ in 0..8 {
                            scope.spawn(|_scope| {
                                generator.create_proofs(black_box(&seed));
                            });
                        }
                    });
                });
            });
        }

        {
            group.throughput(Throughput::Elements(16));
            group.bench_function("proofs/single/16x", |b| {
                b.iter(|| {
                    rayon::scope(|scope| {
                        for _ in 0..16 {
                            scope.spawn(|_scope| {
                                generator.create_proofs(black_box(&seed));
                            });
                        }
                    });
                });
            });
        }
    }

    #[cfg(feature = "parallel")]
    {
        group.throughput(Throughput::Elements(1));
        group.bench_function("proofs/parallel/1x", |b| {
            b.iter(|| {
                generator.create_proofs_parallel(black_box(&seed));
            });
        });

        group.throughput(Throughput::Elements(2));
        group.bench_function("proofs/parallel/2x", |b| {
            b.iter(|| {
                rayon::scope(|scope| {
                    for _ in 0..2 {
                        scope.spawn(|_scope| {
                            generator.create_proofs_parallel(black_box(&seed));
                        });
                    }
                });
            });
        });

        group.throughput(Throughput::Elements(4));
        group.bench_function("proofs/parallel/4x", |b| {
            b.iter(|| {
                rayon::scope(|scope| {
                    for _ in 0..4 {
                        scope.spawn(|_scope| {
                            generator.create_proofs_parallel(black_box(&seed));
                        });
                    }
                });
            });
        });

        group.throughput(Throughput::Elements(8));
        group.bench_function("proofs/parallel/8x", |b| {
            b.iter(|| {
                rayon::scope(|scope| {
                    for _ in 0..8 {
                        scope.spawn(|_scope| {
                            generator.create_proofs_parallel(black_box(&seed));
                        });
                    }
                });
            });
        });

        group.throughput(Throughput::Elements(16));
        group.bench_function("proofs/parallel/16x", |b| {
            b.iter(|| {
                rayon::scope(|scope| {
                    for _ in 0..16 {
                        scope.spawn(|_scope| {
                            generator.create_proofs_parallel(black_box(&seed));
                        });
                    }
                });
            });
        });
    }

    let proofs = generator.create_proofs(&seed);
    let proof = proofs.for_s_bucket(s_bucket_with_proof).unwrap();

    group.throughput(Throughput::Elements(1));
    group.bench_function("verification", |b| {
        b.iter(|| {
            assert!(<PosTable as Table>::is_proof_valid(
                &seed,
                s_bucket_with_proof,
                &proof
            ));
        });
    });
    group.finish();
}

pub fn criterion_benchmark(c: &mut Criterion) {
    {
        // This challenge index with the above seed is known to have a solution
        let s_bucket_with_proof = SBucket::from(31500);

        pos_bench::<ab_proof_of_space::chia::ChiaTable>(c, "chia", s_bucket_with_proof)
    }
    {
        // This challenge index with above seed is known to have a solution
        let s_bucket_with_proof = SBucket::from(0);

        pos_bench::<ab_proof_of_space::shim::ShimTable>(c, "shim", s_bucket_with_proof)
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
