use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::{thread_rng, Rng};
use std::num::NonZeroUsize;
use subspace_archiving::archiver::Archiver;
use subspace_core_primitives::pieces::Record;
use subspace_erasure_coding::ErasureCoding;
use subspace_kzg::Kzg;

const AMOUNT_OF_DATA: usize = 5 * 1024 * 1024;
const SMALL_BLOCK_SIZE: usize = 500;

fn criterion_benchmark(c: &mut Criterion) {
    let mut input = vec![0u8; AMOUNT_OF_DATA];
    thread_rng().fill(input.as_mut_slice());
    let kzg = Kzg::new();
    let erasure_coding = ErasureCoding::new(
        NonZeroUsize::new(Record::NUM_S_BUCKETS.next_power_of_two().ilog2() as usize)
            .expect("Not zero; qed"),
    )
    .unwrap();
    let archiver = Archiver::new(kzg, erasure_coding);

    c.bench_function("segment-archiving-large-block", |b| {
        b.iter(|| {
            archiver
                .clone()
                .add_block(black_box(input.clone()), black_box(Default::default()));
        })
    });

    c.bench_function("segment-archiving-small-blocks", |b| {
        b.iter(|| {
            let mut archiver = archiver.clone();
            for chunk in input.chunks(SMALL_BLOCK_SIZE) {
                archiver.add_block(black_box(chunk.to_vec()), black_box(Default::default()));
            }
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
