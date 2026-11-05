use ab_archiving::archiver::{Archiver, NewArchivedSegment, Segment, SegmentItem};
use ab_archiving::reconstructor::{Reconstructor, ReconstructorError};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, Record};
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

/// Encode a segment the same way the archiver does, with only source pieces present, which is
/// enough to reconstruct it
fn segment_to_pieces(segment: &Segment) -> Vec<Option<Piece>> {
    let encoded_segment = segment.encode();
    assert!(encoded_segment.len() <= RecordedHistorySegment::SIZE);

    let mut source_pieces = iter::repeat_with(Piece::default)
        .take(RecordedHistorySegment::NUM_RAW_RECORDS)
        .collect::<Vec<_>>();
    for (piece, bytes) in source_pieces
        .iter_mut()
        .zip(encoded_segment.chunks(Record::SIZE))
    {
        piece.record.as_flattened_mut()[..bytes.len()].copy_from_slice(bytes);
    }

    source_pieces
        .into_iter()
        .map(Some)
        .chain(iter::repeat_n(
            None,
            ArchivedHistorySegment::NUM_PIECES - RecordedHistorySegment::NUM_RAW_RECORDS,
        ))
        .collect()
}

/// Archive blocks into three segments, such that the second segment contains its parent segment
/// header, the end of the first block, the whole second block and the beginning of the third block,
/// which is a good base for forged segments
fn archive_blocks_for_forging(
    erasure_coding: &ErasureCoding,
) -> ([Vec<u8>; 4], Vec<NewArchivedSegment>) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let mut archiver = Archiver::new(TEST_SHARD_INDEX, erasure_coding.clone());
    let blocks = [
        // Block that overflows into the next segment
        RecordedHistorySegment::SIZE * 3 / 2,
        // Block that fits into the second segment fully and is larger than a segment header
        1024,
        // Block that overflows into the next segment
        RecordedHistorySegment::SIZE,
        // Block that overflows into the next segment
        RecordedHistorySegment::SIZE,
    ]
    .map(|block_size| {
        let mut block = vec![0u8; block_size];
        rng.fill_bytes(block.as_mut_slice());
        block
    });
    let archived_segments = blocks
        .iter()
        .flat_map(|block| {
            archiver
                .add_block(block.clone(), Vec::new())
                .unwrap()
                .archived_segments
        })
        .collect::<Vec<_>>();

    assert_eq!(archived_segments.len(), 3);

    (blocks, archived_segments)
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
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // Only first block fits
        assert!(
            contents.blocks == vec![(BlockNumber::from(0), block_0)],
            "Blocks differ"
        );
        assert_eq!(contents.segment_header, None);
    }

    {
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // Second block is finished, but also third is included
        assert!(
            contents.blocks
                == vec![
                    (BlockNumber::from(1), block_1),
                    (BlockNumber::from(2), block_2.clone())
                ],
            "Blocks differ"
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

        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // Only third block is fully contained
        assert!(
            contents.blocks == vec![(BlockNumber::from(2), block_2)],
            "Blocks differ"
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
    }

    {
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert!(contents.blocks.is_empty());
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

        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert!(contents.blocks.is_empty());
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
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert!(contents.blocks.is_empty());
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
        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert!(contents.blocks.is_empty());
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
        let (_, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Enough data to reconstruct fourth block
        assert!(
            contents.blocks == vec![(BlockNumber::from(3), block_3)],
            "Blocks differ"
        );
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
        let (_, contents) = Reconstructor::new(erasure_coding)
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Nothing is fully contained here
        assert!(contents.blocks.is_empty());
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
        let (_, contents) = Reconstructor::new(erasure_coding.clone())
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

        assert!(
            contents.blocks == vec![(BlockNumber::from(0), block_0.clone())],
            "Blocks differ"
        );
    }

    {
        // Take just parity shards
        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(
                &iter::repeat_n(None, RecordedHistorySegment::NUM_RAW_RECORDS)
                    .chain(pieces.parity_pieces().map(Some))
                    .collect::<Vec<_>>(),
            )
            .unwrap();

        assert!(
            contents.blocks == vec![(BlockNumber::from(0), block_0.clone())],
            "Blocks differ"
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
        let (_, contents) = Reconstructor::new(erasure_coding)
            .add_segment(&pieces)
            .unwrap();

        assert!(
            contents.blocks == vec![(BlockNumber::from(0), block_0)],
            "Blocks differ"
        );
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

        assert_matches!(
            result.err(),
            Some(ReconstructorError::DataShardsReconstruction(_))
        );
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

        assert_matches!(result.err(), Some(ReconstructorError::SegmentDecoding(_)));
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        let result = reconstructor
            .clone()
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::ONE,
                actual_segment_index: LocalSegmentIndex::from(2)
            })
        );

        let (reconstructor, _) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // The same segment again
        let result = reconstructor
            .clone()
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::from(2),
                actual_segment_index: LocalSegmentIndex::ONE
            })
        );

        let result = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::from(2),
                actual_segment_index: LocalSegmentIndex::from(3)
            })
        );
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // The first segment has no parent segment header, but still can't be added twice
        let result = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::ONE,
                actual_segment_index: LocalSegmentIndex::ZERO
            })
        );
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding)
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // The first segment can't follow other segments either
        let result = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::from(2),
                actual_segment_index: LocalSegmentIndex::ZERO
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
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // Only first block fits
        assert_blocks(&contents.blocks, &[(0, &block_0)]);
    }

    for archived_segment in &archived_segments[1..3] {
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segment.pieces))
            .unwrap();

        // Second block continues here and ends exactly with the third segment, which is only known
        // from the next segment
        assert_blocks(&contents.blocks, &[]);

        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segment.pieces))
            .unwrap();

        // Nothing is fully contained here
        assert_blocks(&contents.blocks, &[]);
    }

    {
        let contents;
        (reconstructor, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Second block is finished, and the third block is fully contained
        assert_blocks(&contents.blocks, &[(1, &block_1), (2, &block_2)]);

        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[3].pieces))
            .unwrap();

        // Only third block is fully contained
        assert_blocks(&contents.blocks, &[(2, &block_2)]);
    }

    {
        let (_, contents) = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Previous segment ended with a whole block, only fourth block is fully contained
        assert_blocks(&contents.blocks, &[(3, &block_3)]);

        let (_, contents) = Reconstructor::new(erasure_coding)
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        assert_blocks(&contents.blocks, &[(3, &block_3)]);
    }
}

#[test]
fn misplaced_parent_segment_header() {
    let erasure_coding = ErasureCoding::new();
    let (_, archived_segments) = archive_blocks_for_forging(&erasure_coding);

    let segment_1 = Reconstructor::new(erasure_coding.clone())
        .reconstruct_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
        .unwrap();
    let [
        SegmentItem::ParentSegmentHeader(_),
        SegmentItem::BlockContinuation {
            bytes: _,
            block_objects: _,
        },
        SegmentItem::Block {
            bytes: _,
            block_objects: _,
        },
        SegmentItem::BlockStart {
            bytes: _,
            block_objects: _,
        },
        ..,
    ] = segment_1.items.as_slice()
    else {
        panic!("Unexpected layout of the second segment");
    };
    let parent_segment_header = segment_1.items[0].clone();

    {
        let mut segment = segment_1.clone();
        segment.items.swap(0, 1);

        // Parent segment header after another item
        let result = Reconstructor::new(erasure_coding.clone())
            .add_segment(&segment_to_pieces(&segment))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::UnexpectedParentSegmentHeader)
        );
    }

    // Second parent segment header after padding, both inserted before the end of the first block,
    // with padding instead of the second block
    let second_header_after_padding = {
        let mut segment = segment_1.clone();
        let padding = segment.items[2].encoded_size()
            - parent_segment_header.encoded_size()
            - SegmentItem::Padding.encoded_size();
        segment
            .items
            .splice(2..3, iter::repeat_n(SegmentItem::Padding, padding));
        segment
            .items
            .splice(1..1, [SegmentItem::Padding, parent_segment_header.clone()]);
        segment
    };
    // Second parent segment header that replaces the second block, after the end of the first block
    let second_header_after_block_end = {
        let mut segment = segment_1.clone();
        let padding = segment.items[2].encoded_size() - parent_segment_header.encoded_size();
        segment.items.splice(
            2..3,
            iter::once(parent_segment_header).chain(iter::repeat_n(SegmentItem::Padding, padding)),
        );
        segment
    };

    for segment in [second_header_after_padding, second_header_after_block_end] {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        let result = reconstructor
            .add_segment(&segment_to_pieces(&segment))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::UnexpectedParentSegmentHeader)
        );
    }
}

#[test]
fn segment_index_overflow() {
    let erasure_coding = ErasureCoding::new();
    let ([_, block_1, ..], archived_segments) = archive_blocks_for_forging(&erasure_coding);

    let segment_1 = Reconstructor::new(erasure_coding.clone())
        .reconstruct_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
        .unwrap();
    let SegmentItem::ParentSegmentHeader(segment_header_0) = segment_1.items[0] else {
        panic!("The second segment starts with a parent segment header");
    };
    let with_parent_segment_index = |parent_segment_index: u64| {
        let mut segment = segment_1.clone();
        segment.items[0] = SegmentItem::ParentSegmentHeader(SegmentHeader {
            index: LocalSegmentIndex::from(parent_segment_index).into(),
            ..segment_header_0
        });
        segment_to_pieces(&segment)
    };

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // Segment after the segment with the maximum index
        let result = reconstructor
            .clone()
            .add_segment(&with_parent_segment_index(u64::MAX))
            .err();

        assert_eq!(result, Some(ReconstructorError::SegmentIndexOverflow));

        // Segment with the maximum index
        let result = reconstructor
            .add_segment(&with_parent_segment_index(u64::MAX - 1))
            .err();

        assert_eq!(
            result,
            Some(ReconstructorError::IncorrectSegmentOrder {
                expected_segment_index: LocalSegmentIndex::ONE,
                actual_segment_index: LocalSegmentIndex::from(u64::MAX)
            })
        );
    }

    {
        // Segment after the segment with the maximum index
        let result = Reconstructor::new(erasure_coding.clone())
            .add_segment(&with_parent_segment_index(u64::MAX))
            .err();

        assert_eq!(result, Some(ReconstructorError::SegmentIndexOverflow));

        // Segment with the maximum index is fine on its own
        let (reconstructor, contents) = Reconstructor::new(erasure_coding)
            .add_segment(&with_parent_segment_index(u64::MAX - 1))
            .unwrap();

        assert_blocks(&contents.blocks, &[(1, &block_1)]);

        // But no segment can follow it
        let result = reconstructor
            .clone()
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[2].pieces))
            .err();

        assert_eq!(result, Some(ReconstructorError::SegmentIndexOverflow));

        let result = reconstructor
            .add_segment(&with_parent_segment_index(u64::MAX))
            .err();

        assert_eq!(result, Some(ReconstructorError::SegmentIndexOverflow));
    }
}

#[test]
fn block_number_overflow() {
    let erasure_coding = ErasureCoding::new();
    let ([block_0, block_1, ..], archived_segments) = archive_blocks_for_forging(&erasure_coding);

    let segment_1 = Reconstructor::new(erasure_coding.clone())
        .reconstruct_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
        .unwrap();
    let SegmentItem::ParentSegmentHeader(segment_header_0) = segment_1.items[0] else {
        panic!("The second segment starts with a parent segment header");
    };
    // The second segment with padding instead of the second block, such that the third block
    // starts right after the end of the first block
    let segment_1_without_block_1 = {
        let mut segment = segment_1.clone();
        let padding = segment.items[2].encoded_size();
        segment
            .items
            .splice(2..3, iter::repeat_n(SegmentItem::Padding, padding));
        segment
    };
    let with_last_archived_block =
        |segment: &Segment,
         last_archived_block_number: u64,
         archived_progress: ArchivedBlockProgress| {
            let mut segment = segment.clone();
            segment.items[0] = SegmentItem::ParentSegmentHeader(SegmentHeader {
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::from(last_archived_block_number).into(),
                    archived_progress,
                },
                ..segment_header_0
            });
            segment_to_pieces(&segment)
        };
    let partial_progress = segment_header_0.last_archived_block.archived_progress;
    let complete_progress = ArchivedBlockProgress::new_complete();

    for archived_progress in [partial_progress, complete_progress] {
        // The second block is numbered right after the last archived block, and neither it nor
        // the number after it may be larger than the maximum
        for last_archived_block_number in [u64::MAX, u64::MAX - 1] {
            let result = Reconstructor::new(erasure_coding.clone())
                .add_segment(&with_last_archived_block(
                    &segment_1,
                    last_archived_block_number,
                    archived_progress,
                ))
                .err();

            assert_eq!(result, Some(ReconstructorError::BlockNumberOverflow));
        }

        // The number after the second block is the maximum
        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&with_last_archived_block(
                &segment_1,
                u64::MAX - 2,
                archived_progress,
            ))
            .unwrap();

        assert_blocks(&contents.blocks, &[(u64::MAX - 1, &block_1)]);

        // Without the second block, the third block that starts in this segment is numbered right
        // after the last archived block, and that number may be the maximum
        let result = Reconstructor::new(erasure_coding.clone())
            .add_segment(&with_last_archived_block(
                &segment_1_without_block_1,
                u64::MAX,
                archived_progress,
            ))
            .err();

        assert_eq!(result, Some(ReconstructorError::BlockNumberOverflow));

        let (_, contents) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&with_last_archived_block(
                &segment_1_without_block_1,
                u64::MAX - 1,
                archived_progress,
            ))
            .unwrap();

        assert_blocks(&contents.blocks, &[]);
    }

    // The first block is completed in this segment with the number of the last archived block,
    // followed by the second block
    for last_archived_block_number in [u64::MAX, u64::MAX - 1] {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        let result = reconstructor
            .add_segment(&with_last_archived_block(
                &segment_1,
                last_archived_block_number,
                partial_progress,
            ))
            .err();

        assert_eq!(result, Some(ReconstructorError::BlockNumberOverflow));
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // The number after the second block that follows the first block is the maximum
        let (_, contents) = reconstructor
            .add_segment(&with_last_archived_block(
                &segment_1,
                u64::MAX - 2,
                partial_progress,
            ))
            .unwrap();

        assert_blocks(
            &contents.blocks,
            &[(u64::MAX - 2, &block_0), (u64::MAX - 1, &block_1)],
        );
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding.clone())
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // The first block has the maximum number, so the third block that starts right after it
        // can't be numbered
        let result = reconstructor
            .add_segment(&with_last_archived_block(
                &segment_1_without_block_1,
                u64::MAX,
                partial_progress,
            ))
            .err();

        assert_eq!(result, Some(ReconstructorError::BlockNumberOverflow));
    }

    {
        let (reconstructor, _) = Reconstructor::new(erasure_coding)
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[0].pieces))
            .unwrap();

        // The third block that starts right after the first block has the maximum number
        let (_, contents) = reconstructor
            .add_segment(&with_last_archived_block(
                &segment_1_without_block_1,
                u64::MAX - 1,
                partial_progress,
            ))
            .unwrap();

        assert_blocks(&contents.blocks, &[(u64::MAX - 1, &block_0)]);
    }
}
