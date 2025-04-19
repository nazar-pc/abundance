use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use subspace_archiving::archiver::Archiver;
use subspace_core_primitives::segments::RecordedHistorySegment;
use subspace_erasure_coding::ErasureCoding;

const AMOUNT_OF_DATA: usize = RecordedHistorySegment::SIZE;
const SMALL_BLOCK_SIZE: usize = 500;

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut input = vec![0u8; AMOUNT_OF_DATA];
    rng.fill_bytes(input.as_mut_slice());
    let erasure_coding = ErasureCoding::new();
    let archiver = Archiver::new(erasure_coding);

    c.bench_function("segment-archiving-whole-segment", |b| {
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
