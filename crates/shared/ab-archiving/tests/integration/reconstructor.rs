use ab_archiving::archiver::{Archiver, SegmentItem};
use ab_archiving::reconstructor::{Reconstructor, ReconstructorError};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::Piece;
use ab_core_primitives::segments::{
    ArchivedBlockProgress, ArchivedHistorySegment, LastArchivedBlock, LocalSegmentIndex,
    RecordedHistorySegment, SegmentHeader, SegmentRoot,
};
use ab_core_primitives::shard::ShardIndex;
use ab_erasure_coding::ErasureCoding;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use parity_scale_codec::Encode;
use std::num::NonZeroU32;
use std::{assert_matches, iter};

const TEST_SHARD_INDEX: ShardIndex = ShardIndex::new(ShardIndex::MAX_SHARD_INDEX - 1).unwrap();

fn pieces_to_option_of_pieces(pieces: &ArchivedHistorySegment) -> Vec<Option<Piece>> {
    pieces.pieces().map(Some).collect()
}

/// Compare block numbers and sizes before contents, such that a mismatch doesn't print whole blocks
fn assert_blocks(actual: &[(BlockNumber, Vec<u8>)], expected: &[(u64, &[u8])]) {
    assert_eq!(
        actual
            .iter()
            .map(|(block_number, block)| (*block_number, block.len()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(block_number, block)| (BlockNumber::from(*block_number), block.len()))
            .collect::<Vec<_>>()
    );
    for ((block_number, actual_block), (_, expected_block)) in actual.iter().zip(expected) {
        assert!(
            actual_block == expected_block,
            "Contents of block {block_number} differ"
        );
    }
}

#[test]
fn basic() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(TEST_SHARD_INDEX, erasure_coding.clone());
    // Block that fits into the segment fully
    let block_0 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that overflows into the next segment
    let block_1 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that also fits into the segment fully
    let block_2 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 4];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that occupies multiple segments
    let block_3 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE * 3];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Extra block
    let block_4 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    let archived_segments = archiver
        .add_block(block_0.clone(), Vec::new())
        .unwrap()
        .archived_segments
        .into_iter()
        .chain(
            archiver
                .add_block(block_1.clone(), Vec::new())
                .unwrap()
                .archived_segments,
        )
        .chain(
            archiver
                .add_block(block_2.clone(), Vec::new())
                .unwrap()
                .archived_segments,
        )
        .chain(
            archiver
                .add_block(block_3.clone(), Vec::new())
                .unwrap()
                .archived_segments,
        )
        .chain(
            archiver
                .add_block(block_4, Vec::new())
                .unwrap()
                .archived_segments,
        )
        .collect::<Vec<_>>();

    assert_eq!(archived_segments.len(), 5);

    let mut reconstructor = Reconstructor::new(erasure_coding.clone());

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // Only first block fits
        assert_eq!(contents.blocks, vec![(BlockNumber::from(0), block_0)]);
        assert_eq!(contents.segment_header, None);
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // Second block is finished, but also third is included
        assert_eq!(
            contents.blocks,
            vec![
                (BlockNumber::from(1), block_1),
                (BlockNumber::from(2), block_2.clone())
            ]
        );
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::ZERO
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(1).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(67_108_854).unwrap()
                ),
            }
        );

        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // Only third block is fully contained
        assert_eq!(contents.blocks, vec![(BlockNumber::from(2), block_2)]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::ZERO
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(1).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(67_108_854).unwrap()
                ),
            }
        );
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_eq!(contents.blocks, vec![]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::ONE
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(33_554_322).unwrap()
                ),
            }
        );

        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_eq!(contents.blocks, vec![]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::ONE
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(33_554_322).unwrap()
                ),
            }
        );
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_eq!(contents.blocks, vec![]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::from(2)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(167_771_960).unwrap()
                ),
            }
        );
    }

    {
        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_eq!(contents.blocks, vec![]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::from(2)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(167_771_960).unwrap()
                ),
            }
        );
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Enough data to reconstruct fourth block
        assert_eq!(contents.blocks, vec![(BlockNumber::from(3), block_3)]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::from(3)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(301_989_598).unwrap()
                ),
            }
        );
    }

    {
        let mut partial_reconstructor = Reconstructor::new(erasure_coding);
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_eq!(contents.blocks, vec![]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().index.as_inner(),
            LocalSegmentIndex::from(3)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::from(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(301_989_598).unwrap()
                ),
            }
        );
    }
}

#[test]
fn partial_data() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(TEST_SHARD_INDEX, erasure_coding.clone());
    // Block that fits into the segment fully
    let block_0 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that overflows into the next segment
    let block_1 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    let archived_segments = archiver
        .add_block(block_0.clone(), Vec::new())
        .unwrap()
        .archived_segments
        .into_iter()
        .chain(
            archiver
                .add_block(block_1, Vec::new())
                .unwrap()
                .archived_segments,
        )
        .collect::<Vec<_>>();

    assert_eq!(archived_segments.len(), 1);

    let pieces = archived_segments.into_iter().next().unwrap().pieces;

    {
        // Take just source shards
        let contents = Reconstructor::new(erasure_coding.clone())
            .add_segment(
                &pieces
                    .source_pieces()
                    .map(Some)
                    .chain(iter::repeat_n(
                        None,
                        RecordedHistorySegment::NUM_RAW_RECORDS,
                    ))
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        assert_eq!(
            contents.blocks,
            vec![(BlockNumber::from(0), block_0.clone())]
        );
    }

    {
        // Take just parity shards
        let contents = Reconstructor::new(erasure_coding.clone())
            .add_segment(
                &iter::repeat_n(None, RecordedHistorySegment::NUM_RAW_RECORDS)
                    .chain(pieces.parity_pieces().map(Some))
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        assert_eq!(
            contents.blocks,
            vec![(BlockNumber::from(0), block_0.clone())]
        );
    }

    {
        // Mix of data and parity shards
        let mut pieces = pieces.pieces().map(Some).collect::<Vec<_>>();
        pieces[ArchivedHistorySegment::NUM_PIECES / 4..]
            .iter_mut()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
            .for_each(|piece| {
                piece.take();
            });
        let contents = Reconstructor::new(erasure_coding)
            .add_segment(&pieces)
            .unwrap();

        assert_eq!(contents.blocks, vec![(BlockNumber::from(0), block_0)]);
    }
}

#[test]
fn invalid_usage() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(TEST_SHARD_INDEX, erasure_coding.clone());
    // Block that overflows into the next segments
    let block_0 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE * 4];
        rng.fill_bytes(block.as_mut_slice());
        block
    };

    let archived_segments = archiver
        .add_block(block_0, Vec::new())
        .unwrap()
        .archived_segments;

    assert_eq!(archived_segments.len(), 4);

    {
        // Not enough shards with contents
        let result = Reconstructor::new(erasure_coding.clone()).add_segment(
            &archived_segments[0]
                .pieces
                .pieces()
                .take(RecordedHistorySegment::NUM_RAW_RECORDS - 1)
                .map(Some)
                .chain(iter::repeat(None))
                .take(ArchivedHistorySegment::NUM_PIECES)
                .collect::<Vec<_>>(),
        );

        assert_matches!(result, Err(ReconstructorError::DataShardsReconstruction(_)));
    }

    {
        // Garbage data
        let result = Reconstructor::new(erasure_coding.clone()).add_segment(
            &iter::repeat_with(|| {
                let mut piece = Piece::default();
                rng.fill_bytes(piece.as_mut());
                Some(piece)
            })
            .take(ArchivedHistorySegment::NUM_PIECES)
            .collect::<Vec<_>>(),
        );

        assert_matches!(result, Err(ReconstructorError::SegmentDecoding(_)));
    }

    {
        let mut reconstructor = Reconstructor::new(erasure_coding);

        reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        let result =
            reconstructor.add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces));

        assert_eq!(
            result,
            Err(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::ONE,
                actual_segment_index: LocalSegmentIndex::from(2)
            })
        );

        reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        let result =
            reconstructor.add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces));

        assert_eq!(
            result,
            Err(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::from(2),
                actual_segment_index: LocalSegmentIndex::from(3)
            })
        );
    }
}

#[test]
fn blocks_completed_at_segment_end() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(TEST_SHARD_INDEX, erasure_coding.clone());
    let segment_header_item_size = SegmentItem::ParentSegmentHeader(SegmentHeader {
        index: LocalSegmentIndex::ZERO.into(),
        root: SegmentRoot::default(),
        prev_segment_header_hash: Blake3Hash::default(),
        last_archived_block: LastArchivedBlock {
            number: BlockNumber::ZERO.into(),
            archived_progress: ArchivedBlockProgress::new_complete(),
        },
    })
    .encoded_size();
    // Segment item enum variant and bytes length
    let block_item_overhead = 1 + size_of::<u32>();
    // Block bytes that fit into a segment with a parent segment header and a single block item
    let segment_block_bytes =
        RecordedHistorySegment::SIZE - segment_header_item_size - block_item_overhead;
    // Block that fits into the segment fully
    let block_0 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    let block_1_bytes_in_segment_0 =
        RecordedHistorySegment::SIZE - block_item_overhead * 2 - block_0.len();
    // Block that fills the rest of the first segment and the whole second and third segments
    let block_1 = {
        let mut block = vec![0u8; block_1_bytes_in_segment_0 + segment_block_bytes * 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that fills the whole fourth segment
    let block_2 = {
        let mut block = vec![0u8; segment_block_bytes];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that fits into the fifth segment fully
    let block_3 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // Block that overflows into the next segment
    let block_4 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    let archived_segments = [&block_0, &block_1, &block_2, &block_3, &block_4]
        .into_iter()
        .flat_map(|block| {
            archiver
                .add_block(block.clone(), Vec::new())
                .unwrap()
                .archived_segments
        })
        .collect::<Vec<_>>();

    let partial = |archived_bytes: usize| {
        ArchivedBlockProgress::new_partial(
            NonZeroU32::new(u32::try_from(archived_bytes).unwrap()).unwrap(),
        )
    };

    // Third and fourth segments end exactly with the end of a block
    assert_eq!(
        archived_segments
            .iter()
            .map(|archived_segment| {
                let LastArchivedBlock {
                    number,
                    archived_progress,
                } = archived_segment.segment_header.last_archived_block;

                (number.as_inner(), archived_progress)
            })
            .collect::<Vec<_>>(),
        vec![
            (BlockNumber::from(1), partial(block_1_bytes_in_segment_0)),
            (
                BlockNumber::from(1),
                partial(block_1_bytes_in_segment_0 + segment_block_bytes)
            ),
            (BlockNumber::from(1), ArchivedBlockProgress::new_complete()),
            (BlockNumber::from(2), ArchivedBlockProgress::new_complete()),
            (
                BlockNumber::from(4),
                partial(segment_block_bytes - block_item_overhead - block_3.len())
            ),
        ]
    );

    let mut reconstructor = Reconstructor::new(erasure_coding.clone());

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // Only first block fits
        assert_blocks(&contents.blocks, &[(0, &block_0)]);
    }

    for archived_segment in &archived_segments[1..3] {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segment.pieces))
            .unwrap();

        // Second block continues here and ends exactly with the third segment, which is only known
        // from the next segment
        assert_blocks(&contents.blocks, &[]);

        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segment.pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_blocks(&contents.blocks, &[]);
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Second block is finished, and the third block is fully contained
        assert_blocks(&contents.blocks, &[(1, &block_1), (2, &block_2)]);

        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Only third block is fully contained
        assert_blocks(&contents.blocks, &[(2, &block_2)]);
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Previous segment ended with a whole block, only fourth block is fully contained
        assert_blocks(&contents.blocks, &[(3, &block_3)]);

        let mut partial_reconstructor = Reconstructor::new(erasure_coding);
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        assert_blocks(&contents.blocks, &[(3, &block_3)]);
    }
}
