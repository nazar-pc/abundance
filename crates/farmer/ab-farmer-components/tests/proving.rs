// Plotting and proving a sector takes too long under Miri
#![cfg(not(miri))]

use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceOffset};
use ab_core_primitives::pos::PosSeed;
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{ShardMembershipEntropy, SolutionRange};
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_components::auditing::{AuditResult, audit_sector_sync};
use ab_farmer_components::plotting::{
    CpuRecordsEncoder, PlotSectorOptions, PlottedSector, plot_sector,
};
use ab_farmer_components::proving::ProvingError;
use ab_farmer_components::sector::SectorMetadataChecksummed;
use ab_farmer_components::shard_commitment::ShardCommitmentsRootsCache;
use ab_proof_of_space::shim::{ShimTable, ShimTableGenerator};
use ab_proof_of_space::{Table, TableGenerator};
use async_lock::Mutex as AsyncMutex;
use futures::executor::block_on;
use std::num::{NonZeroU16, NonZeroU64};
use std::slice;
use std::sync::atomic::AtomicBool;

const PIECES_IN_SECTOR: u16 = 1;
const PUBLIC_KEY_HASH: Blake3Hash = Blake3Hash::new([0; _]);
const SECTOR_INDEX: SectorIndex = SectorIndex::ZERO;

fn shard_commitments_roots_cache() -> ShardCommitmentsRootsCache {
    ShardCommitmentsRootsCache::new(Blake3Hash::default())
}

fn plot_test_sector(
    erasure_coding: &ErasureCoding,
    table_generator: &ShimTableGenerator,
    history_size: HistorySize,
) -> (Vec<u8>, PlottedSector) {
    let shard_commitments_root = &shard_commitments_roots_cache().get(history_size);
    let farmer_protocol_info = FarmerProtocolInfo {
        history_size,
        max_pieces_in_sector: PIECES_IN_SECTOR,
        recent_segments: HistorySize::new(NonZeroU64::new(5).unwrap()),
        recent_history_fraction: (
            HistorySize::new(NonZeroU64::new(1).unwrap()),
            HistorySize::new(NonZeroU64::new(10).unwrap()),
        ),
        min_sector_lifetime: HistorySize::new(NonZeroU64::new(4).unwrap()),
    };

    // Contents of pieces don't matter for these tests
    let sector_id = SectorId::new(
        &PUBLIC_KEY_HASH,
        shard_commitments_root,
        SECTOR_INDEX,
        history_size,
    );
    let piece_getter = (PieceOffset::ZERO..)
        .take(PIECES_IN_SECTOR.into())
        .map(|piece_offset| {
            let piece_index = sector_id.derive_piece_index(
                piece_offset,
                history_size,
                farmer_protocol_info.max_pieces_in_sector,
                farmer_protocol_info.recent_segments,
                farmer_protocol_info.recent_history_fraction,
            );
            (piece_index, Piece::default())
        })
        .collect::<Vec<_>>();

    let mut sector = Vec::new();
    let plotted_sector = block_on(plot_sector(PlotSectorOptions {
        public_key_hash: &PUBLIC_KEY_HASH,
        shard_commitments_root,
        sector_index: SECTOR_INDEX,
        piece_getter: &piece_getter,
        farmer_protocol_info,
        erasure_coding,
        pieces_in_sector: PIECES_IN_SECTOR,
        sector_output: &mut sector,
        downloading_semaphore: None,
        encoding_semaphore: None,
        records_encoder: &mut CpuRecordsEncoder::<ShimTable>::new(
            slice::from_ref(table_generator),
            erasure_coding,
            &AsyncMutex::default(),
        ),
        abort_early: &AtomicBool::new(false),
    }))
    .unwrap();

    (sector, plotted_sector)
}

/// Find a global challenge for which the audited s-bucket is not empty according to metadata
fn find_global_challenge(sector_metadata: &SectorMetadataChecksummed) -> Blake3Hash {
    let sector_id = SectorId::new(
        &PUBLIC_KEY_HASH,
        &shard_commitments_roots_cache().get(sector_metadata.history_size),
        sector_metadata.sector_index,
        sector_metadata.history_size,
    );

    (0..=u8::MAX)
        .map(|byte| Blake3Hash::new([byte; _]))
        .find(|global_challenge| {
            let s_bucket = sector_id
                .derive_sector_slot_challenge(global_challenge)
                .s_bucket_audit_index();
            sector_metadata.s_bucket_sizes[usize::from(s_bucket)] > 0
        })
        .unwrap()
}

fn audit_test_sector<'a>(
    shard_commitments_roots_cache: &'a ShardCommitmentsRootsCache,
    sector: &'a [u8],
    sector_metadata: &'a SectorMetadataChecksummed,
    global_challenge: &Blake3Hash,
) -> AuditResult<'a, &'a [u8]> {
    audit_sector_sync(
        &PUBLIC_KEY_HASH,
        shard_commitments_roots_cache,
        ShardMembershipEntropy::default(),
        NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
        global_challenge,
        SolutionRange::MAX,
        sector,
        sector_metadata,
    )
    .unwrap()
    .unwrap()
}

#[test]
fn proving_rejects_s_bucket_size_mismatch() {
    let erasure_coding = &ErasureCoding::new();
    let table_generator = &ShimTable::generator();
    let shard_commitments_roots_cache = &shard_commitments_roots_cache();
    let (sector, plotted_sector) = plot_test_sector(
        erasure_coding,
        table_generator,
        HistorySize::new(NonZeroU64::MIN),
    );
    let sector_metadata = &plotted_sector.sector_metadata;
    let global_challenge = &find_global_challenge(sector_metadata);
    let s_bucket = plotted_sector
        .sector_id
        .derive_sector_slot_challenge(global_challenge)
        .s_bucket_audit_index();

    // Consistent metadata produces a solution
    let mut solutions = audit_test_sector(
        shard_commitments_roots_cache,
        &sector,
        sector_metadata,
        global_challenge,
    )
    .solution_candidates
    .into_solutions(erasure_coding, |seed: &PosSeed| {
        table_generator.create_proofs(seed)
    })
    .unwrap();
    solutions.next().unwrap().unwrap();

    // Metadata with one more chunk in the audited s-bucket than the sector contents map has
    let mut wrong_sector_metadata = sector_metadata.clone();
    wrong_sector_metadata.s_bucket_sizes[usize::from(s_bucket)] += 1;
    let result = audit_test_sector(
        shard_commitments_roots_cache,
        &sector,
        &wrong_sector_metadata,
        global_challenge,
    )
    .solution_candidates
    .into_solutions(erasure_coding, |seed: &PosSeed| {
        table_generator.create_proofs(seed)
    })
    .err();
    let expected_size = sector_metadata.s_bucket_sizes[usize::from(s_bucket)];
    assert!(
        matches!(
            result,
            Some(ProvingError::SBucketSizeMismatch {
                s_bucket: actual_s_bucket,
                expected,
                actual,
            }) if actual_s_bucket == s_bucket
                && expected == expected_size + 1
                && actual == usize::from(expected_size)
        ),
        "{result:?}"
    );
}
