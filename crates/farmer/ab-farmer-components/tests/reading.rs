// Proofs and chunks of a full-size record take too long to process under Miri
#![cfg(not(miri))]

use ab_core_primitives::pieces::{PieceOffset, Record, RecordChunk};
use ab_core_primitives::pos::PosSeed;
use ab_core_primitives::sectors::{SBucket, SectorIndex};
use ab_core_primitives::segments::HistorySize;
use ab_farmer_components::ReadAt;
use ab_farmer_components::reading::{ReadingError, SectorRecordChunks, read_sector_record_chunks};
use ab_farmer_components::sector::{FoundProofs, SectorContentsMap, SectorMetadata, sector_size};
use ab_proof_of_space::shim::ShimTable;
use ab_proof_of_space::{PosProofs, Table, TableGenerator};
use futures::executor::block_on;
use std::assert_matches;
use std::num::NonZeroU64;

const PIECES_IN_SECTOR: u16 = 1;

fn create_test_proofs() -> Box<PosProofs> {
    ShimTable::generator().create_proofs(&PosSeed::from([1; PosSeed::SIZE]))
}

fn flip_bit(found_proofs: &mut FoundProofs, s_bucket: SBucket) {
    let s_bucket = usize::from(s_bucket);
    found_proofs[s_bucket / u8::BITS as usize] ^= 1 << (s_bucket % u8::BITS as usize);
}

/// Reads the record at `piece_offset` from a sector made of zeroes, whose map has
/// `record_chunks_used` for its only record
fn read_test_record_chunks(
    piece_offset: PieceOffset,
    record_chunks_used: &FoundProofs,
    pos_proofs: &PosProofs,
) -> Result<SectorRecordChunks, ReadingError> {
    let mut sector_contents_map = SectorContentsMap::new(PIECES_IN_SECTOR);
    sector_contents_map.iter_record_chunks_used_mut()[0] = *record_chunks_used;
    let s_bucket_offsets = SectorMetadata {
        sector_index: SectorIndex::ZERO,
        pieces_in_sector: PIECES_IN_SECTOR,
        s_bucket_sizes: sector_contents_map.s_bucket_sizes(),
        history_size: HistorySize::new(NonZeroU64::MIN),
    }
    .s_bucket_offsets();
    let sector = vec![0; sector_size(PIECES_IN_SECTOR)];

    block_on(read_sector_record_chunks(
        piece_offset,
        PIECES_IN_SECTOR,
        &s_bucket_offsets,
        &sector_contents_map,
        pos_proofs,
        &ReadAt::from_sync(sector.as_slice()),
    ))
}

#[test]
fn reading_accepts_map_that_matches_proofs() {
    let pos_proofs = create_test_proofs();

    let sector_record_chunks =
        read_test_record_chunks(PieceOffset::ZERO, &pos_proofs.found_proofs, &pos_proofs).unwrap();

    // Chunks in the sector are zeroes, so each one is decoded into the hash of its proof
    let s_buckets_with_proofs = (SBucket::ZERO..=SBucket::MAX).filter_map(|s_bucket| {
        pos_proofs
            .for_s_bucket(s_bucket)
            .map(|pos_proof| (s_bucket, pos_proof))
    });
    for (s_bucket, pos_proof) in s_buckets_with_proofs {
        assert_eq!(
            sector_record_chunks.get_chunk(s_bucket),
            RecordChunk::from(*pos_proof.hash()),
            "{s_bucket}"
        );
    }
    assert_eq!(
        sector_record_chunks.present.source.count() + sector_record_chunks.present.parity.count(),
        Record::NUM_CHUNKS
    );
}

#[test]
fn reading_rejects_map_that_does_not_match_proofs() {
    let pos_proofs = create_test_proofs();
    let first_with_proof = (SBucket::ZERO..=SBucket::MAX)
        .find(|&s_bucket| pos_proofs.for_s_bucket(s_bucket).is_some())
        .unwrap();
    let first_without_proof = (SBucket::ZERO..=SBucket::MAX)
        .find(|&s_bucket| pos_proofs.for_s_bucket(s_bucket).is_none())
        .unwrap();

    // A missing chunk at an s-bucket with a proof, then an extra chunk at an s-bucket without one
    for s_bucket in [first_with_proof, first_without_proof] {
        let mut record_chunks_used = pos_proofs.found_proofs;
        flip_bit(&mut record_chunks_used, s_bucket);

        let error =
            read_test_record_chunks(PieceOffset::ZERO, &record_chunks_used, &pos_proofs).err();
        assert_matches!(
            error,
            Some(ReadingError::PosProofsMismatch { piece_offset })
                if piece_offset == PieceOffset::ZERO,
            "{s_bucket}: {error:?}"
        );
    }
}

#[test]
fn reading_rejects_piece_offset_outside_of_sector() {
    let pos_proofs = create_test_proofs();
    let piece_offset = PieceOffset::from(PIECES_IN_SECTOR);

    let error = read_test_record_chunks(piece_offset, &pos_proofs.found_proofs, &pos_proofs).err();
    assert_matches!(
        error,
        Some(ReadingError::PosProofsMismatch {
            piece_offset: actual_piece_offset
        }) if actual_piece_offset == piece_offset,
        "{error:?}"
    );
}
