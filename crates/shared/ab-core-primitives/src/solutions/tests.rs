use crate::block::BlockNumber;
use crate::hashes::Blake3Hash;
use crate::pieces::{PieceIndex, PieceOffset, PiecePosition, RecordProof, RecordRoot};
use crate::segments::{
    HistorySize, LocalSegmentIndex, RecordedHistorySegment, SegmentIndex, SegmentPosition,
    SegmentRoot, ShardSegmentRootWithPosition, SuperSegment, SuperSegmentHeader, SuperSegmentIndex,
    SuperSegmentRoot,
};
use crate::shard::ShardIndex;
use crate::solutions::{Solution, SolutionVerifyError, SolutionVerifyPieceParams};
use ab_merkle_tree::blake3::BalancedMerkleTree;
use blake3::OUT_LEN;
use core::array;
use core::num::NonZeroU64;

const MAX_PIECES_IN_SECTOR: u16 = 1000;
const RECENT_SEGMENTS: HistorySize = HistorySize::new(NonZeroU64::new(5).unwrap());
const RECENT_HISTORY_FRACTION: (HistorySize, HistorySize) = (
    HistorySize::new(NonZeroU64::new(1).unwrap()),
    HistorySize::new(NonZeroU64::new(10).unwrap()),
);
/// Large enough for some pieces to be picked from recent history only
const HISTORY_SIZE: HistorySize = HistorySize::new(NonZeroU64::new(60).unwrap());
/// Max segment index of the super segment before the one with the last segments of history
const PREVIOUS_MAX_SEGMENT_INDEX: SegmentIndex = SegmentIndex::from(56);

/// A segment with arbitrary record roots instead of the ones of actual records, which is enough
/// for piece verification
struct TestSegment {
    shard_index: ShardIndex,
    local_segment_index: LocalSegmentIndex,
    record_roots: [[u8; OUT_LEN]; RecordedHistorySegment::NUM_PIECES],
    root: SegmentRoot,
    record_proofs: Vec<RecordProof>,
}

impl TestSegment {
    fn new(shard_index: ShardIndex, local_segment_index: u64, seed: u8) -> Self {
        let record_roots = array::from_fn(|position| {
            let mut record_root = [seed; _];
            record_root[0] = position as u8;
            record_root
        });
        let merkle_tree =
            BalancedMerkleTree::<{ RecordedHistorySegment::NUM_PIECES }>::new(&record_roots);

        Self {
            shard_index,
            local_segment_index: LocalSegmentIndex::from(local_segment_index),
            record_roots,
            root: SegmentRoot::from(merkle_tree.root()),
            record_proofs: merkle_tree.all_proofs().map(RecordProof::from).collect(),
        }
    }
}

/// Super segment with `segments` after the one with `previous_max_segment_index`
fn new_super_segment(
    previous_max_segment_index: SegmentIndex,
    segments: &[TestSegment],
) -> SuperSegment {
    SuperSegment::new(
        &SuperSegmentHeader {
            index: SuperSegmentIndex::ONE.into(),
            root: SuperSegmentRoot::default(),
            prev_super_segment_header_hash: Blake3Hash::default(),
            max_segment_index: previous_max_segment_index.into(),
            target_beacon_chain_block_number: BlockNumber::ONE.into(),
            num_segments: 1,
        },
        BlockNumber::from(10),
        segments
            .iter()
            .zip(0..)
            .map(|(segment, segment_position)| ShardSegmentRootWithPosition {
                shard_index: segment.shard_index,
                segment_position: SegmentPosition::from(segment_position),
                local_segment_index: segment.local_segment_index,
                segment_root: segment.root,
            })
            .collect(),
    )
    .unwrap()
}

fn verify_piece_params(super_segment_header: &SuperSegmentHeader) -> SolutionVerifyPieceParams {
    SolutionVerifyPieceParams {
        max_pieces_in_sector: MAX_PIECES_IN_SECTOR,
        super_segment_root: super_segment_header.root,
        num_segments: super_segment_header.num_segments,
        max_segment_index: super_segment_header.max_segment_index.as_inner(),
        recent_segments: RECENT_SEGMENTS,
        recent_history_fraction: RECENT_HISTORY_FRACTION,
        min_sector_lifetime: HistorySize::new(NonZeroU64::new(4).unwrap()),
        current_history_size: HISTORY_SIZE,
        sector_expiration_check_super_segment_root: None,
    }
}

/// Solution that uses the piece at `piece_position` of the segment at `segment_position` in the
/// super segment
fn solution_for_piece(
    solution: &Solution,
    super_segment: &SuperSegment,
    segments: &[TestSegment],
    segment_position: SegmentPosition,
    piece_position: PiecePosition,
) -> Solution {
    let segment = &segments[u32::from(segment_position) as usize];

    Solution {
        piece_shard_index: segment.shard_index,
        piece_local_segment_index: segment.local_segment_index,
        segment_root: segment.root,
        segment_proof: super_segment.proof_for_segment(segment_position).unwrap(),
        record_root: RecordRoot::from(segment.record_roots[usize::from(piece_position)]),
        record_proof: segment.record_proofs[usize::from(piece_position)],
        ..*solution
    }
}

/// Piece offsets of a sector at which pieces of `segment_index` are required, with the piece
/// indices.
///
/// The first is in the part of the sector with pieces from recent history only, the second is not.
fn piece_offsets(
    solution: &Solution,
    segment_index: SegmentIndex,
) -> [(PieceOffset, PieceIndex); 2] {
    let sector_id = solution.sector_id();
    let find_piece_offset = |recent_history_only: bool| {
        (PieceOffset::ZERO..)
            .take(usize::from(MAX_PIECES_IN_SECTOR))
            // Odd piece offsets in the first `2 * MAX_PIECES_IN_SECTOR / 10` (the recent history
            // fraction) of the sector, see `SectorId::derive_piece_index()`
            .filter(|&piece_offset| {
                let piece_offset = u16::from(piece_offset);
                (piece_offset < 2 * MAX_PIECES_IN_SECTOR / 10 && piece_offset % 2 == 1)
                    == recent_history_only
            })
            .find_map(|piece_offset| {
                let piece_index = sector_id.derive_piece_index(
                    piece_offset,
                    solution.history_size,
                    MAX_PIECES_IN_SECTOR,
                    RECENT_SEGMENTS,
                    RECENT_HISTORY_FRACTION,
                );

                (piece_index.segment_index() == segment_index)
                    .then_some((piece_offset, piece_index))
            })
            .unwrap()
    };

    [find_piece_offset(true), find_piece_offset(false)]
}

#[test]
fn verify_piece() {
    // Segments of different shards in one super segment, which contains the last segments of
    // history
    let segments = [
        TestSegment::new(ShardIndex::BEACON_CHAIN, 40, 1),
        TestSegment::new(ShardIndex::new(1).unwrap(), 7, 2),
        TestSegment::new(ShardIndex::BEACON_CHAIN, 41, 3),
    ];
    let super_segment = new_super_segment(PREVIOUS_MAX_SEGMENT_INDEX, &segments);
    let params = verify_piece_params(&super_segment.header);
    assert_eq!(
        super_segment.header.max_segment_index.as_inner(),
        HISTORY_SIZE.segment_index()
    );

    // A super segment that doesn't contain any segments of the above
    let other_segments = [TestSegment::new(ShardIndex::BEACON_CHAIN, 42, 4)];
    let other_super_segment = new_super_segment(
        super_segment.header.max_segment_index.as_inner(),
        &other_segments,
    );
    let other_params = verify_piece_params(&other_super_segment.header);

    let solution = Solution {
        history_size: HISTORY_SIZE,
        ..Solution::genesis_solution()
    };

    for segment_position in (0..segments.len() as u32).map(SegmentPosition::from) {
        let segment_index = PREVIOUS_MAX_SEGMENT_INDEX
            + SegmentIndex::ONE
            + SegmentIndex::from(u64::from(segment_position));

        for (piece_offset, piece_index) in piece_offsets(&solution, segment_index) {
            let solution = Solution {
                piece_offset,
                ..solution
            };
            let piece_position = piece_index.position();

            let correct_solution = solution_for_piece(
                &solution,
                &super_segment,
                &segments,
                segment_position,
                piece_position,
            );
            assert_eq!(correct_solution.verify_piece(&params), Ok(()));

            // A piece at the same position, but in a different segment of the same super segment
            for other_segment_position in (0..segments.len() as u32)
                .map(SegmentPosition::from)
                .filter(|&other_segment_position| other_segment_position != segment_position)
            {
                let other_solution = solution_for_piece(
                    &solution,
                    &super_segment,
                    &segments,
                    other_segment_position,
                    piece_position,
                );
                assert_eq!(
                    other_solution.verify_piece(&params),
                    Err(SolutionVerifyError::SegmentNotInSuperSegment)
                );
            }

            // A super segment that doesn't contain the segment of the piece
            assert_eq!(
                correct_solution.verify_piece(&other_params),
                Err(SolutionVerifyError::PieceSegmentNotInSuperSegment {
                    segment_index,
                    max_segment_index: other_super_segment.header.max_segment_index.as_inner(),
                    num_segments: 1,
                })
            );
            // Even with a piece from it at the same position
            let other_solution = solution_for_piece(
                &solution,
                &other_super_segment,
                &other_segments,
                SegmentPosition::ZERO,
                piece_position,
            );
            assert_eq!(
                other_solution.verify_piece(&other_params),
                Err(SolutionVerifyError::PieceSegmentNotInSuperSegment {
                    segment_index,
                    max_segment_index: other_super_segment.header.max_segment_index.as_inner(),
                    num_segments: 1,
                })
            );
        }
    }
}
