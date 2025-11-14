#![feature(exact_size_is_empty)]

use ab_archiving::archiver::Archiver;
use ab_core_primitives::ed25519::Ed25519PublicKey;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pos::PosSeed;
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::segments::{HistorySize, RecordedHistorySegment};
use ab_core_primitives::solutions::SolutionRange;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_components::auditing::audit_plot_sync;
use ab_farmer_components::file_ext::{FileExt, OpenOptionsExt};
use ab_farmer_components::plotting::{
    CpuRecordsEncoder, PlotSectorOptions, PlottedSector, plot_sector,
};
use ab_farmer_components::sector::{
    SectorContentsMap, SectorMetadata, SectorMetadataChecksummed, sector_size,
};
use ab_proof_of_space::chia::ChiaTable;
use ab_proof_of_space::{Table, TableGenerator};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use futures::executor::block_on;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::hint::black_box;
use std::io::Write;
use std::num::NonZeroU64;
use std::{env, fs, slice};

type PosTable = ChiaTable;

const MAX_PIECES_IN_SECTOR: u16 = 1000;

pub fn criterion_benchmark(c: &mut Criterion) {
    println!("Initializing...");
    let base_path = env::var("BASE_PATH")
        .map(|base_path| base_path.parse().unwrap())
        .unwrap_or_else(|_error| env::temp_dir());
    let pieces_in_sector = env::var("PIECES_IN_SECTOR")
        .map(|base_path| base_path.parse().unwrap())
        .unwrap_or_else(|_error| MAX_PIECES_IN_SECTOR);
    let persist_sector = env::var("PERSIST_SECTOR")
        .map(|persist_sector| persist_sector == "1")
        .unwrap_or_else(|_error| false);
    let sectors_count = env::var("SECTORS_COUNT")
        .map(|sectors_count| sectors_count.parse().unwrap())
        .unwrap_or(10);

    let public_key = &Ed25519PublicKey::default();
    let public_key_hash = &public_key.hash();
    let sector_index = SectorIndex::ZERO;
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut input = RecordedHistorySegment::new_boxed();
    rng.fill_bytes(input.as_mut().as_mut());
    let erasure_coding = &ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding.clone());
    let table_generator = PosTable::generator();
    let archived_history_segment = archiver
        .add_block(
            AsRef::<[u8]>::as_ref(input.as_ref()).to_vec(),
            Default::default(),
        )
        .unwrap()
        .archived_segments
        .into_iter()
        .next()
        .unwrap();

    let farmer_protocol_info = FarmerProtocolInfo {
        history_size: HistorySize::new(NonZeroU64::new(1).unwrap()),
        max_pieces_in_sector: pieces_in_sector,
        recent_segments: HistorySize::new(NonZeroU64::new(5).unwrap()),
        recent_history_fraction: (
            HistorySize::new(NonZeroU64::new(1).unwrap()),
            HistorySize::new(NonZeroU64::new(10).unwrap()),
        ),
        min_sector_lifetime: HistorySize::new(NonZeroU64::new(4).unwrap()),
    };
    let solution_range = SolutionRange::MAX;

    let sector_size = sector_size(pieces_in_sector);

    let persisted_sector = base_path.join(format!("subspace_bench_sector_{pieces_in_sector}.plot"));

    let (plotted_sector, plotted_sector_bytes) = if persist_sector && persisted_sector.is_file() {
        println!(
            "Reading persisted sector from {}...",
            persisted_sector.display()
        );

        let plotted_sector_bytes = fs::read(&persisted_sector).unwrap();
        let sector_contents_map = SectorContentsMap::from_bytes(
            &plotted_sector_bytes[..SectorContentsMap::encoded_size(pieces_in_sector)],
            pieces_in_sector,
        )
        .unwrap();
        let sector_metadata = SectorMetadataChecksummed::from(SectorMetadata {
            sector_index,
            pieces_in_sector,
            s_bucket_sizes: sector_contents_map.s_bucket_sizes(),
            history_size: farmer_protocol_info.history_size,
        });

        (
            PlottedSector {
                sector_id: SectorId::new(
                    public_key_hash,
                    sector_index,
                    farmer_protocol_info.history_size,
                ),
                sector_index,
                sector_metadata,
                piece_indexes: vec![],
            },
            plotted_sector_bytes,
        )
    } else {
        println!("Plotting one sector...");

        let mut plotted_sector_bytes = Vec::new();

        let plotted_sector = block_on(plot_sector(PlotSectorOptions {
            public_key_hash,
            sector_index,
            piece_getter: &archived_history_segment,
            farmer_protocol_info,
            erasure_coding,
            pieces_in_sector,
            sector_output: &mut plotted_sector_bytes,
            downloading_semaphore: black_box(None),
            encoding_semaphore: black_box(None),
            records_encoder: &mut CpuRecordsEncoder::<PosTable>::new(
                slice::from_ref(&table_generator),
                erasure_coding,
                &Default::default(),
            ),
            abort_early: &Default::default(),
        }))
        .unwrap();

        (plotted_sector, plotted_sector_bytes)
    };

    assert_eq!(plotted_sector_bytes.len(), sector_size);

    if persist_sector && !persisted_sector.is_file() {
        println!(
            "Writing persisted sector into {}...",
            persisted_sector.display()
        );
        fs::write(persisted_sector, &plotted_sector_bytes).unwrap()
    }

    println!("Searching for solutions");
    let (global_challenge, solution_candidates) = &loop {
        let mut global_challenge = Blake3Hash::default();
        rng.fill_bytes(global_challenge.as_mut());

        let audit_results = audit_plot_sync(
            public_key_hash,
            &global_challenge,
            solution_range,
            &plotted_sector_bytes,
            slice::from_ref(&plotted_sector.sector_metadata),
            &HashSet::default(),
        )
        .unwrap();

        let solution_candidates = match audit_results.into_iter().next() {
            Some(audit_result) => audit_result.solution_candidates,
            None => {
                continue;
            }
        };

        if !solution_candidates
            .clone()
            .into_solutions(erasure_coding, |seed: &PosSeed| {
                table_generator.create_proofs_parallel(seed)
            })
            .unwrap()
            .is_empty()
        {
            break (global_challenge, solution_candidates);
        }
    };

    let mut group = c.benchmark_group("proving");
    {
        group.throughput(Throughput::Elements(1));
        group.bench_function("memory", |b| {
            b.iter(|| {
                solution_candidates
                    .clone()
                    .into_solutions(
                        black_box(erasure_coding),
                        black_box(|seed: &PosSeed| table_generator.create_proofs_parallel(seed)),
                    )
                    .unwrap()
                    // Process just one solution
                    .next()
                    .unwrap()
                    .unwrap();
            })
        });
    }

    {
        println!("Writing {sectors_count} sectors to disk...");

        let plot_file_path = base_path.join("subspace_bench_plot.plot");
        let mut plot_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .advise_random_access()
            .open(&plot_file_path)
            .unwrap();

        plot_file
            .preallocate(sector_size as u64 * sectors_count)
            .unwrap();
        plot_file.advise_random_access().unwrap();

        for _i in 0..sectors_count {
            plot_file
                .write_all(plotted_sector_bytes.as_slice())
                .unwrap();
        }

        let sectors_metadata = (0..sectors_count)
            .map(|_| plotted_sector.sector_metadata.clone())
            .collect::<Vec<_>>();

        {
            let plot_file = &plot_file;

            let audit_results = audit_plot_sync(
                public_key_hash,
                global_challenge,
                solution_range,
                &plot_file,
                &sectors_metadata,
                &HashSet::default(),
            )
            .unwrap();
            let solution_candidates = audit_results
                .into_iter()
                .map(|audit_result| audit_result.solution_candidates)
                .collect::<Vec<_>>();

            group.throughput(Throughput::Elements(sectors_count));
            group.bench_function("disk", |b| {
                b.iter_batched(
                    || solution_candidates.clone(),
                    |solution_candidates| {
                        for solution_candidates in solution_candidates {
                            solution_candidates
                                .into_solutions(
                                    black_box(erasure_coding),
                                    black_box(|seed: &PosSeed| {
                                        table_generator.create_proofs_parallel(seed)
                                    }),
                                )
                                .unwrap()
                                // Process just one solution
                                .next()
                                .unwrap()
                                .unwrap();
                        }
                    },
                    BatchSize::LargeInput,
                );
            });
        }

        drop(plot_file);
        fs::remove_file(plot_file_path).unwrap();
    }
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
