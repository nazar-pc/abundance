use ab_core_primitives::pieces::{Record, RecordChunk};
use ab_core_primitives::segments::RecordedHistorySegment;
use ab_erasure_coding::{ErasureCoding, ShardsPresent};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    // Many small shards: erasure coding of record chunks, as done during archiving, plotting and
    // farming
    erasure_coding::<{ Record::NUM_CHUNKS }, { RecordChunk::SIZE }>(c, "record");
    // Few large shards: erasure coding of records within a segment, as done when archiving history
    // and reconstructing it back
    erasure_coding::<{ RecordedHistorySegment::NUM_RAW_RECORDS }, { Record::SIZE }>(c, "segment");
}

fn boxed_shards<const NUM_SHARDS: usize, const SHARD_BYTES: usize>()
-> Box<[[u8; SHARD_BYTES]; NUM_SHARDS]> {
    // SAFETY: Data structure filled with zeroes is a valid invariant
    unsafe { Box::<[[u8; SHARD_BYTES]; NUM_SHARDS]>::new_zeroed().assume_init() }
}

fn erasure_coding<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
    c: &mut Criterion,
    name: &str,
) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let mut source_shards = boxed_shards::<NUM_SHARDS, SHARD_BYTES>();
    for shard in &mut *source_shards {
        rng.fill_bytes(shard);
    }
    let mut parity_shards = boxed_shards::<NUM_SHARDS, SHARD_BYTES>();

    let erasure_coding = ErasureCoding::new();

    c.bench_function(&format!("{name}/extend"), |b| {
        b.iter(|| {
            erasure_coding
                .extend(black_box(&*source_shards), black_box(&mut *parity_shards))
                .unwrap();
        });
    });

    erasure_coding
        .extend(&source_shards, &mut parity_shards)
        .unwrap();

    // Half of the shards missing is the worst case that is still recoverable with 1/2 erasure
    // coding rate
    let num_missing = NUM_SHARDS / 2;
    let mut present = ShardsPresent::<NUM_SHARDS>::all();
    for index in 0..num_missing {
        present.source.unset(index);
        // Exactly as many parity shards as necessary to recover the missing source shards, the
        // rest is not even looked at
        present.parity.unset(index + num_missing);
    }

    let mut recovered_source_shards = source_shards.clone();
    let mut recovered_parity_shards = parity_shards.clone();

    // Recover source shards only, the way it is done when reconstructing a segment or reading a
    // record from a sector
    c.bench_function(&format!("{name}/recover-source"), |b| {
        b.iter(|| {
            erasure_coding
                .recover_source(
                    black_box(&mut *recovered_source_shards),
                    black_box(&parity_shards),
                    black_box(&present),
                )
                .unwrap();
        });
    });

    // Recover both source and parity shards, the way it is done when recovering all record chunks
    // of a piece or reconstructing a whole piece
    c.bench_function(&format!("{name}/recover-all"), |b| {
        b.iter(|| {
            erasure_coding
                .recover_all(
                    black_box(&mut *recovered_source_shards),
                    black_box(&mut *recovered_parity_shards),
                    black_box(&present),
                )
                .unwrap();
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
