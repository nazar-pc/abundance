use ab_core_primitives::pieces::{Record, RecordChunk};
use ab_core_primitives::segments::RecordedHistorySegment;
use ab_erasure_coding::{ErasureCoding, RecoveryShardState};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::iter;

fn criterion_benchmark(c: &mut Criterion) {
    // Many small shards: erasure coding of record chunks, as done during archiving, plotting and
    // farming
    erasure_coding(c, "record", Record::NUM_CHUNKS, RecordChunk::SIZE);
    // Few large shards: erasure coding of records within a segment, as done when archiving history
    // and reconstructing it back
    erasure_coding(
        c,
        "segment",
        RecordedHistorySegment::NUM_RAW_RECORDS,
        Record::SIZE,
    );
}

fn erasure_coding(c: &mut Criterion, name: &str, num_shards: usize, shard_size: usize) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let source_shards = iter::repeat_with(|| {
        let mut shard = vec![0u8; shard_size].into_boxed_slice();
        rng.fill_bytes(&mut shard);
        shard
    })
    .take(num_shards)
    .collect::<Vec<_>>();
    let mut parity_shards = vec![vec![0u8; shard_size].into_boxed_slice(); num_shards];

    let erasure_coding = ErasureCoding::new();

    c.bench_function(&format!("{name}/extend"), |b| {
        b.iter(|| {
            erasure_coding
                .extend(
                    black_box(source_shards.iter()),
                    black_box(parity_shards.iter_mut()),
                )
                .unwrap();
        });
    });

    erasure_coding
        .extend(source_shards.iter(), parity_shards.iter_mut())
        .unwrap();

    // Half of the shards missing is the worst case that is still recoverable with 1/2 erasure
    // coding rate
    let num_missing = num_shards / 2;
    let mut recovered_source_shards = vec![vec![0u8; shard_size].into_boxed_slice(); num_shards];
    let mut recovered_parity_shards = vec![vec![0u8; shard_size].into_boxed_slice(); num_shards];

    // Recover source shards only, the way it is done when reconstructing a segment or reading a
    // record from a sector
    c.bench_function(&format!("{name}/recover-source"), |b| {
        b.iter(|| {
            let source = source_shards
                .iter()
                .zip(recovered_source_shards.iter_mut())
                .enumerate()
                .map(|(index, (input, output))| {
                    if index < num_missing {
                        RecoveryShardState::MissingRecover(output.as_mut())
                    } else {
                        RecoveryShardState::Present(input.as_ref())
                    }
                });
            // Exactly as many parity shards as necessary to recover the missing source shards, the
            // rest is not even looked at
            let parity = parity_shards.iter().enumerate().map(|(index, input)| {
                if index < num_missing {
                    RecoveryShardState::Present(input.as_ref())
                } else {
                    RecoveryShardState::MissingIgnore
                }
            });

            erasure_coding
                .recover(black_box(source), black_box(parity))
                .unwrap();
        });
    });

    // Recover both source and parity shards, the way it is done when recovering all record chunks
    // of a piece or reconstructing a whole piece
    c.bench_function(&format!("{name}/recover-all"), |b| {
        b.iter(|| {
            let source = source_shards
                .iter()
                .zip(recovered_source_shards.iter_mut())
                .enumerate()
                .map(|(index, (input, output))| {
                    if index < num_missing {
                        RecoveryShardState::MissingRecover(output.as_mut())
                    } else {
                        RecoveryShardState::Present(input.as_ref())
                    }
                });
            let parity = parity_shards
                .iter()
                .zip(recovered_parity_shards.iter_mut())
                .enumerate()
                .map(|(index, (input, output))| {
                    if index < num_missing {
                        RecoveryShardState::Present(input.as_ref())
                    } else {
                        RecoveryShardState::MissingRecover(output.as_mut())
                    }
                });

            erasure_coding
                .recover(black_box(source), black_box(parity))
                .unwrap();
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
