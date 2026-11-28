use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, Record};
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::solutions::ShardCommitmentHash;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::plotting::RecordsEncoder;
use ab_proof_of_space_gpu::{Device, DeviceType};
use chacha20::ChaCha8Rng;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use futures::executor::block_on;
use rand::prelude::*;
use std::env;
use std::hint::black_box;
use std::num::NonZeroU8;
use std::sync::atomic::AtomicBool;
use triomphe::Arc;

fn criterion_benchmark(c: &mut Criterion) {
    println!("Initializing...");
    // Enough records to keep all queues busy, but not a full sector to keep iterations short
    let pieces_in_sector = env::var("PIECES_IN_SECTOR")
        .map_or(16_u16, |pieces_in_sector| pieces_in_sector.parse().unwrap());
    let record_encoding_concurrency = env::var("GPU_RECORD_ENCODING_CONCURRENCY")
        .ok()
        .map(|concurrency| concurrency.parse::<NonZeroU8>().unwrap());

    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let abort_early = AtomicBool::new(false);
    let sector_id = SectorId::new(
        &Blake3Hash::default(),
        &ShardCommitmentHash::default(),
        SectorIndex::ZERO,
        HistorySize::ONE,
    );
    let mut records = Record::new_zero_vec(usize::from(pieces_in_sector));
    for record in &mut records {
        rng.fill_bytes(record.as_flattened_mut());
    }

    // Several queues per device are needed to fully utilize the GPU
    let devices = block_on(Device::enumerate(|device_type| {
        record_encoding_concurrency.unwrap_or(match device_type {
            DeviceType::DiscreteGpu => NonZeroU8::new(4).unwrap(),
            DeviceType::Other
            | DeviceType::IntegratedGpu
            | DeviceType::VirtualGpu
            | DeviceType::Cpu => NonZeroU8::new(2).unwrap(),
        })
    }));

    let mut group = c.benchmark_group(format!("encoding[{pieces_in_sector}]"));
    group.throughput(Throughput::ElementsAndBytes {
        elements: u64::from(pieces_in_sector),
        bytes: u64::from(pieces_in_sector) * Piece::SIZE as u64,
    });
    for device in devices {
        // Too slow to be useful
        if device.device_type() == DeviceType::Cpu {
            println!(
                "Skipping GPU device emulated by the CPU: {}: {}",
                device.id(),
                device.name()
            );
            continue;
        }

        let mut records_encoder = device
            .instantiate(erasure_coding.clone(), Arc::default())
            .unwrap();

        group.bench_function(format!("{}: {}", device.id(), device.name()), |b| {
            b.iter(|| {
                records_encoder
                    .encode_records(
                        black_box(&sector_id),
                        black_box(&mut records),
                        black_box(&abort_early),
                    )
                    .unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
