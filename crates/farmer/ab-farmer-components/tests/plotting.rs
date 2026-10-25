// Sectors are made of full-size records, which takes too long to process under Miri
#![cfg(not(miri))]

use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceOffset, Record};
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::segments::HistorySize;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::FarmerProtocolInfo;
use ab_farmer_components::plotting::{
    DownloadSectorOptions, DownloadedSector, EncodeSectorOptions, PlottingError, RecordsEncoder,
    download_sector, encode_sector, write_sector,
};
use ab_farmer_components::sector::SectorContentsMap;
use ab_farmer_components::shard_commitment::ShardCommitmentsRootsCache;
use futures::executor::block_on;
use std::assert_matches;
use std::num::NonZeroU64;
use std::sync::atomic::AtomicBool;

const PIECES_IN_SECTOR: u16 = 2;

/// Returns the given map instead of encoding anything
struct MapRecordsEncoder(SectorContentsMap);

impl RecordsEncoder for MapRecordsEncoder {
    fn encode_records(
        &mut self,
        _sector_id: &SectorId,
        _records: &mut [Record],
        _abort_early: &AtomicBool,
    ) -> anyhow::Result<SectorContentsMap> {
        Ok(self.0.clone())
    }
}

fn download_test_sector(erasure_coding: &ErasureCoding) -> DownloadedSector {
    let public_key_hash = &Blake3Hash::default();
    let history_size = HistorySize::new(NonZeroU64::MIN);
    let shard_commitments_root =
        &ShardCommitmentsRootsCache::new(Blake3Hash::default()).get(history_size);
    let sector_index = SectorIndex::ZERO;
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
        public_key_hash,
        shard_commitments_root,
        sector_index,
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

    block_on(download_sector(DownloadSectorOptions {
        public_key_hash,
        shard_commitments_root,
        sector_index,
        piece_getter: &piece_getter,
        farmer_protocol_info,
        erasure_coding,
        pieces_in_sector: PIECES_IN_SECTOR,
    }))
    .unwrap()
}

/// Map with the first `Record::NUM_CHUNKS` chunks used in each record
fn valid_map(pieces_in_sector: u16) -> SectorContentsMap {
    let mut sector_contents_map = SectorContentsMap::new(pieces_in_sector);
    for record_chunks_used in sector_contents_map.iter_record_chunks_used_mut() {
        record_chunks_used
            .iter_mut()
            .take(Record::NUM_CHUNKS / u8::BITS as usize)
            .for_each(|bits| *bits = u8::MAX);
    }
    sector_contents_map
}

fn encode_test_sector(
    erasure_coding: &ErasureCoding,
    sector_contents_map: SectorContentsMap,
    abort_early: bool,
) -> Result<(), PlottingError> {
    let encoded_sector = encode_sector(
        download_test_sector(erasure_coding),
        EncodeSectorOptions {
            sector_index: SectorIndex::ZERO,
            records_encoder: &mut MapRecordsEncoder(sector_contents_map),
            abort_early: &AtomicBool::new(abort_early),
        },
    )?;

    write_sector(&encoded_sector, &mut Vec::new())
}

#[test]
fn encode_sector_accepts_valid_map() {
    let erasure_coding = &ErasureCoding::new();

    encode_test_sector(erasure_coding, valid_map(PIECES_IN_SECTOR), false).unwrap();
}

#[test]
fn encode_sector_rejects_wrong_number_of_records() {
    let erasure_coding = &ErasureCoding::new();

    for pieces_in_sector in [PIECES_IN_SECTOR - 1, PIECES_IN_SECTOR + 1] {
        let result = encode_test_sector(erasure_coding, valid_map(pieces_in_sector), false);
        assert_matches!(
            result,
            Err(PlottingError::BadSectorContentsMapSize { provided, expected })
                if provided == usize::from(pieces_in_sector)
                    && expected == usize::from(PIECES_IN_SECTOR),
            "{pieces_in_sector}: {result:?}"
        );
    }
}

#[test]
fn encode_sector_rejects_wrong_number_of_chunks_used() {
    let erasure_coding = &ErasureCoding::new();
    let first_unused_byte = Record::NUM_CHUNKS / u8::BITS as usize;

    // One more chunk in the first record
    let mut too_many = valid_map(PIECES_IN_SECTOR);
    too_many.iter_record_chunks_used_mut()[0][first_unused_byte] = 1;
    // One less chunk in the second record
    let mut too_few = valid_map(PIECES_IN_SECTOR);
    too_few.iter_record_chunks_used_mut()[1][0] = u8::MAX - 1;
    // Both of the above, which keeps the total number of chunks the same as in a valid map
    let mut balanced = too_many.clone();
    balanced.iter_record_chunks_used_mut()[1][0] = u8::MAX - 1;

    for (sector_contents_map, expected_piece_offset, expected_provided) in [
        (too_many, PieceOffset::ZERO, Record::NUM_CHUNKS + 1),
        (too_few, PieceOffset::from(1), Record::NUM_CHUNKS - 1),
        (balanced, PieceOffset::ZERO, Record::NUM_CHUNKS + 1),
    ] {
        let result = encode_test_sector(erasure_coding, sector_contents_map, false);
        assert_matches!(
            result,
            Err(PlottingError::BadRecordChunksUsed {
                piece_offset,
                provided,
                expected: Record::NUM_CHUNKS,
            }) if piece_offset == expected_piece_offset
                && usize::try_from(provided).unwrap() == expected_provided,
            "{expected_piece_offset}: {result:?}"
        );
    }
}

#[test]
fn encode_sector_aborts_early_with_incomplete_map() {
    let erasure_coding = &ErasureCoding::new();

    // An encoder that stopped early may leave records without chunks
    let result = encode_test_sector(
        erasure_coding,
        SectorContentsMap::new(PIECES_IN_SECTOR),
        true,
    );
    assert_matches!(result, Err(PlottingError::AbortEarly), "{result:?}");
}
