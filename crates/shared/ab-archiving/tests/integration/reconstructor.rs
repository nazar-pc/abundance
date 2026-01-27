use ab_archiving::archiver::Archiver;
use ab_archiving::reconstructor::{Reconstructor, ReconstructorError};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::pieces::{FlatPieces, Piece};
use ab_core_primitives::segments::{
    ArchivedBlockProgress, ArchivedHistorySegment, LastArchivedBlock, LocalSegmentIndex,
    RecordedHistorySegment,
};
use ab_erasure_coding::ErasureCoding;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::assert_matches::assert_matches;
use std::iter;
use std::num::NonZeroU32;

fn pieces_to_option_of_pieces(pieces: &FlatPieces) -> Vec<Option<Piece>> {
    pieces.pieces().map(Some).collect()
}

#[test]
fn basic() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding.clone());
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
        assert_eq!(contents.blocks, vec![(BlockNumber::new(0), block_0)]);
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
                (BlockNumber::new(1), block_1),
                (BlockNumber::new(2), block_2.clone())
            ]
        );
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::ZERO
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(1).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(67108854).unwrap()
                ),
            }
        );

        let mut partial_reconstructor = Reconstructor::new(erasure_coding.clone());
        let contents = partial_reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[1].pieces))
            .unwrap();

        // Only third block is fully contained
        assert_eq!(contents.blocks, vec![(BlockNumber::new(2), block_2)]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::ZERO
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(1).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(67108854).unwrap()
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
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::ONE
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(33554322).unwrap()
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
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::ONE
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(33554322).unwrap()
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
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::from(2)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(167771960).unwrap()
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
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::from(2)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(167771960).unwrap()
                ),
            }
        );
    }

    {
        let contents = reconstructor
            .add_segment(&pieces_to_option_of_pieces(&archived_segments[4].pieces))
            .unwrap();

        // Enough data to reconstruct fourth block
        assert_eq!(contents.blocks, vec![(BlockNumber::new(3), block_3)]);
        assert!(contents.segment_header.is_some());
        assert_eq!(
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::from(3)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(301989598).unwrap()
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
            contents.segment_header.unwrap().local_segment_index(),
            LocalSegmentIndex::from(3)
        );
        assert_eq!(
            contents.segment_header.unwrap().last_archived_block,
            LastArchivedBlock {
                number: BlockNumber::new(3).into(),
                archived_progress: ArchivedBlockProgress::new_partial(
                    NonZeroU32::new(301989598).unwrap()
                ),
            }
        );
    }
}

#[test]
fn partial_data() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding.clone());
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
            vec![(BlockNumber::new(0), block_0.clone())]
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
            vec![(BlockNumber::new(0), block_0.clone())]
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

        assert_eq!(contents.blocks, vec![(BlockNumber::new(0), block_0)]);
    }
}

#[test]
fn invalid_usage() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding.clone());
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
