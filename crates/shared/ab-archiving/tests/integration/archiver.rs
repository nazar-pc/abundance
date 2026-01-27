use ab_archiving::archiver::{Archiver, ArchiverInstantiationError, SegmentItem};
use ab_archiving::objects::{BlockObject, GlobalObject};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, Record};
use ab_core_primitives::segments::{
    ArchivedBlockProgress, ArchivedHistorySegment, LastArchivedBlock, RecordedHistorySegment,
    SegmentHeader, SegmentIndex, SegmentRoot,
};
use ab_erasure_coding::ErasureCoding;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::assert_matches::assert_matches;
use std::io::Write;
use std::iter;
use std::num::NonZeroU32;

fn extract_data<O: Into<u32>>(data: &[u8], offset: O) -> &[u8] {
    let offset: u32 = offset.into();
    let size = u32::decode(&mut &data[offset as usize..]).unwrap();
    &data[offset as usize + u32::encoded_fixed_size().unwrap()..][..size as usize]
}

fn extract_data_from_source_record<O: Into<u32>>(record: &Record, offset: O) -> &[u8] {
    let offset: u32 = offset.into();
    let size = u32::decode(&mut &record.as_flattened()[offset as usize..]).unwrap();
    &record.as_flattened()[offset as usize + u32::encoded_fixed_size().unwrap()..][..size as usize]
}

#[track_caller]
fn compare_block_objects_to_global_objects<'a>(
    block_objects: impl Iterator<Item = (&'a [u8], &'a BlockObject)>,
    global_objects: impl Iterator<Item = (Piece, GlobalObject)>,
) {
    block_objects.zip(global_objects).for_each(
        |((block, block_object_mapping), (piece, global_object_mapping))| {
            assert_eq!(
                extract_data_from_source_record(piece.record(), global_object_mapping.offset),
                extract_data(block, block_object_mapping.offset)
            );
        },
    );
}

#[test]
fn archiver() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding.clone());

    let (block_0, block_0_block_objects) = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 2];
        rng.fill_bytes(block.as_mut_slice());

        block[0..].as_mut().write_all(&100_u32.encode()).unwrap();
        block[RecordedHistorySegment::SIZE / 3..]
            .as_mut()
            .write_all(&128_u32.encode())
            .unwrap();
        let block_objects = vec![
            BlockObject {
                hash: Blake3Hash::default(),
                offset: 0u32,
            },
            BlockObject {
                hash: Blake3Hash::default(),
                offset: RecordedHistorySegment::SIZE as u32 / 3,
            },
        ];

        (block, block_objects)
    };
    let block_0_outcome = archiver
        .add_block(block_0.clone(), block_0_block_objects.clone())
        .unwrap();
    let archived_segments = block_0_outcome.archived_segments;
    let object_mapping = block_0_outcome.global_objects.clone();
    // There is not enough data to produce archived segment yet
    assert!(archived_segments.is_empty());
    // All block mappings must appear in the global object mapping
    assert_eq!(object_mapping.len(), block_0_block_objects.len());

    let (block_1, block_1_block_objects) = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE / 3 * 2];
        rng.fill_bytes(block.as_mut_slice());

        block[RecordedHistorySegment::SIZE / 6..]
            .as_mut()
            .write_all(&100_u32.encode())
            .unwrap();
        block[RecordedHistorySegment::SIZE / 5..]
            .as_mut()
            .write_all(&2048_u32.encode())
            .unwrap();
        block[RecordedHistorySegment::SIZE / 3 * 2 - 200..]
            .as_mut()
            .write_all(&100_u32.encode())
            .unwrap();
        let block_objects = vec![
            BlockObject {
                hash: Blake3Hash::default(),
                offset: RecordedHistorySegment::SIZE as u32 / 6,
            },
            BlockObject {
                hash: Blake3Hash::default(),
                offset: RecordedHistorySegment::SIZE as u32 / 5,
            },
            BlockObject {
                hash: Blake3Hash::default(),
                offset: RecordedHistorySegment::SIZE as u32 / 3 * 2 - 200,
            },
        ];
        (block, block_objects)
    };
    // This should produce 1 archived segment
    let block_1_outcome = archiver
        .add_block(block_1.clone(), block_1_block_objects.clone())
        .unwrap();
    let archived_segments = block_1_outcome.archived_segments;
    let object_mapping = block_1_outcome.global_objects.clone();
    assert_eq!(archived_segments.len(), 1);

    let first_archived_segment = archived_segments.first().cloned().unwrap();
    assert_eq!(
        first_archived_segment.pieces.len(),
        ArchivedHistorySegment::NUM_PIECES
    );
    assert_eq!(
        first_archived_segment.segment_header.segment_index(),
        SegmentIndex::ZERO
    );
    assert_eq!(
        first_archived_segment
            .segment_header
            .prev_segment_header_hash,
        Blake3Hash::default(),
    );
    {
        let last_archived_block = first_archived_segment.segment_header.last_archived_block;
        assert_eq!(last_archived_block.number(), BlockNumber::ONE);
        assert_eq!(
            last_archived_block.partial_archived(),
            Some(NonZeroU32::new(67108854).unwrap())
        );
    }

    // All block mappings must appear in the global object mapping
    assert_eq!(object_mapping.len(), block_1_block_objects.len());
    {
        // 4 objects fit into the first segment, 2 from block 0 and 2 from block 1
        let block_objects = iter::repeat(block_0.as_ref())
            .zip(&block_0_block_objects)
            .chain(iter::repeat(block_1.as_ref()).zip(&block_1_block_objects))
            .take(4);
        let global_objects = block_0_outcome
            .global_objects
            .into_iter()
            .chain(object_mapping)
            .take(4)
            .map(|object_mapping| {
                (
                    Piece::from(
                        &first_archived_segment.pieces
                            [object_mapping.piece_index.position() as usize],
                    ),
                    object_mapping,
                )
            });

        compare_block_objects_to_global_objects(block_objects, global_objects);
    }

    #[cfg(not(feature = "parallel"))]
    let iter = first_archived_segment.pieces.iter().enumerate();
    #[cfg(feature = "parallel")]
    let iter = first_archived_segment.pieces.par_iter().enumerate();
    let results = iter
        .map(|(position, piece)| {
            (
                position,
                piece.is_valid(
                    &first_archived_segment.segment_header.segment_root,
                    position as u32,
                ),
            )
        })
        .collect::<Vec<_>>();
    for (position, valid) in results {
        assert!(valid, "Piece at position {position} is valid");
    }

    let block_2 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE * 2];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    // This should be big enough to produce two archived segments in one go
    let block_2_outcome = archiver.add_block(block_2.clone(), Vec::new()).unwrap();
    let archived_segments = block_2_outcome.archived_segments.clone();
    let object_mapping = block_2_outcome.global_objects.clone();
    assert_eq!(archived_segments.len(), 2);

    // Check that initializing archiver with initial state before last block results in the same
    // archived segments once last block is added.
    {
        let mut archiver_with_initial_state = Archiver::with_initial_state(
            erasure_coding.clone(),
            first_archived_segment.segment_header,
            &block_1,
            block_1_block_objects.clone(),
        )
        .unwrap();

        let initial_block_2_outcome = archiver_with_initial_state
            .add_block(block_2.clone(), Vec::new())
            .unwrap();

        // The rest of block 1 doesn't create any segments by itself
        assert_eq!(
            initial_block_2_outcome.archived_segments,
            block_2_outcome.archived_segments
        );

        // The rest of block 1 doesn't create any segments, but it does have the final block 1
        // object mapping. And there are no mappings in block 2.
        assert_eq!(initial_block_2_outcome.global_objects.len(), 1);
        assert_eq!(
            initial_block_2_outcome.global_objects[0],
            block_1_outcome.global_objects[2]
        );
    }

    // No block mappings should appear in the global object mapping
    assert_eq!(object_mapping.len(), 0);
    // 1 object fits into the second segment
    // There are no objects left for the third segment
    assert_eq!(
        block_1_outcome.global_objects[2]
            .piece_index
            .segment_index(),
        archived_segments[0].segment_header.segment_index(),
    );
    {
        let block_objects =
            iter::repeat(block_1.as_ref()).zip(block_1_block_objects.iter().skip(2));
        let global_objects = object_mapping.into_iter().map(|object_mapping| {
            (
                Piece::from(
                    &archived_segments[0].pieces[object_mapping.piece_index.position() as usize],
                ),
                object_mapping,
            )
        });

        compare_block_objects_to_global_objects(block_objects, global_objects);
    }

    // Check archived bytes for block with index `2` in each archived segment
    {
        let archived_segment = archived_segments.first().unwrap();
        let last_archived_block = archived_segment.segment_header.last_archived_block;
        assert_eq!(last_archived_block.number(), BlockNumber::new(2));
        assert_eq!(
            last_archived_block.partial_archived(),
            Some(NonZeroU32::new(111848003).unwrap())
        );
    }
    {
        let archived_segment = archived_segments.get(1).unwrap();
        let last_archived_block = archived_segment.segment_header.last_archived_block;
        assert_eq!(last_archived_block.number(), BlockNumber::new(2));
        assert_eq!(
            last_archived_block.partial_archived(),
            Some(NonZeroU32::new(246065641).unwrap())
        );
    }

    // Check that both archived segments have expected content and valid pieces in them
    let mut expected_segment_index = SegmentIndex::ONE;
    let mut previous_segment_header_hash = first_archived_segment.segment_header.hash();
    let last_segment_header = archived_segments.iter().last().unwrap().segment_header;
    for archived_segment in archived_segments {
        assert_eq!(
            archived_segment.pieces.len(),
            ArchivedHistorySegment::NUM_PIECES
        );
        assert_eq!(
            archived_segment.segment_header.segment_index(),
            expected_segment_index
        );
        assert_eq!(
            archived_segment.segment_header.prev_segment_header_hash,
            previous_segment_header_hash
        );

        #[cfg(not(feature = "parallel"))]
        let iter = archived_segment.pieces.iter().enumerate();
        #[cfg(feature = "parallel")]
        let iter = archived_segment.pieces.par_iter().enumerate();
        let results = iter
            .map(|(position, piece)| {
                (
                    position,
                    piece.is_valid(
                        &archived_segment.segment_header.segment_root,
                        position as u32,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (position, valid) in results {
            assert!(valid, "Piece at position {position} is valid");
        }

        expected_segment_index += SegmentIndex::ONE;
        previous_segment_header_hash = archived_segment.segment_header.hash();
    }

    // Add a block such that it fits in the next segment exactly
    let block_3 = {
        let mut block = vec![0u8; RecordedHistorySegment::SIZE - 22369914];
        rng.fill_bytes(block.as_mut_slice());
        block
    };
    let block_3_outcome = archiver.add_block(block_3.clone(), Vec::new()).unwrap();
    let archived_segments = block_3_outcome.archived_segments.clone();
    let object_mapping = block_3_outcome.global_objects.clone();
    assert_eq!(archived_segments.len(), 1);

    // There are no objects left for the fourth segment
    assert_eq!(object_mapping.len(), 0);

    // Check that initializing archiver with initial state before last block results in the same
    // archived segments and mappings once last block is added
    {
        let mut archiver_with_initial_state =
            Archiver::with_initial_state(erasure_coding, last_segment_header, &block_2, Vec::new())
                .unwrap();

        let initial_block_3_outcome = archiver_with_initial_state
            .add_block(block_3, Vec::new())
            .unwrap();

        // The rest of block 2 doesn't create any segments by itself
        assert_eq!(
            initial_block_3_outcome.archived_segments,
            block_3_outcome.archived_segments,
        );

        // The rest of block 2 doesn't have any mappings
        assert_eq!(
            initial_block_3_outcome.global_objects,
            block_3_outcome.global_objects
        );
    }

    // Block should fit exactly into the last archived segment (rare case)
    {
        let archived_segment = archived_segments.first().unwrap();
        let last_archived_block = archived_segment.segment_header.last_archived_block;
        assert_eq!(last_archived_block.number(), BlockNumber::new(3));
        assert_eq!(last_archived_block.partial_archived(), None);

        #[cfg(not(feature = "parallel"))]
        let iter = archived_segment.pieces.iter().enumerate();
        #[cfg(feature = "parallel")]
        let iter = archived_segment.pieces.par_iter().enumerate();
        let results = iter
            .map(|(position, piece)| {
                (
                    position,
                    piece.is_valid(
                        &archived_segment.segment_header.segment_root,
                        position as u32,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (position, valid) in results {
            assert!(valid, "Piece at position {position} is valid");
        }
    }
}

#[test]
fn invalid_usage() {
    let erasure_coding = ErasureCoding::new();
    {
        assert!(
            Archiver::new(erasure_coding.clone())
                .add_block(Vec::new(), Vec::new())
                .is_none(),
            "Empty block is not allowed"
        );
    }
    {
        let result = Archiver::with_initial_state(
            erasure_coding.clone(),
            SegmentHeader {
                segment_index: SegmentIndex::ZERO.into(),
                segment_root: SegmentRoot::default(),
                prev_segment_header_hash: Blake3Hash::default(),
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::ZERO.into(),
                    archived_progress: ArchivedBlockProgress::new_partial(
                        NonZeroU32::new(10).unwrap(),
                    ),
                },
            },
            &[0u8; 10],
            Vec::new(),
        );

        assert_matches!(
            result,
            Err(ArchiverInstantiationError::InvalidLastArchivedBlock(_)),
        );

        if let Err(ArchiverInstantiationError::InvalidLastArchivedBlock(size)) = result {
            assert_eq!(size, 10);
        }
    }

    {
        let result = Archiver::with_initial_state(
            erasure_coding.clone(),
            SegmentHeader {
                segment_index: SegmentIndex::ZERO.into(),
                segment_root: SegmentRoot::default(),
                prev_segment_header_hash: Blake3Hash::default(),
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::ZERO.into(),
                    archived_progress: ArchivedBlockProgress::new_partial(
                        NonZeroU32::new(10).unwrap(),
                    ),
                },
            },
            &[0u8; 6],
            Vec::new(),
        );

        assert_matches!(
            result,
            Err(ArchiverInstantiationError::InvalidBlockSmallSize { .. }),
        );

        if let Err(ArchiverInstantiationError::InvalidBlockSmallSize {
            block_bytes,
            archived_block_bytes,
        }) = result
        {
            assert_eq!(block_bytes, 6);
            assert_eq!(archived_block_bytes, 10);
        }
    }
}

#[test]
fn early_segment_creation() {
    let erasure_coding = ErasureCoding::new();

    // Carefully compute the block size such that there is just 5 bytes left to fill the segment,
    // but this should already produce an archived segment because just enum variant and the
    // smallest segment item encoding will take 6 bytes to encode
    let block_size = RecordedHistorySegment::SIZE
        - 2 * (
            // Enum variant
            1
            // Byte length encoding
            + u32::encoded_fixed_size().unwrap()
        );
    assert_eq!(
        Archiver::new(erasure_coding.clone())
            .add_block(vec![0u8; block_size], Vec::new())
            .unwrap()
            .archived_segments
            .len(),
        1
    );

    // Cutting just one byte and block length is no longer large enough to produce a segment because
    // 6 bytes is enough for one more segment item
    assert!(
        Archiver::new(erasure_coding)
            .add_block(vec![0u8; block_size - 1], Vec::new())
            .unwrap()
            .archived_segments
            .is_empty()
    );
}

#[test]
fn object_on_the_edge_of_segment() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let erasure_coding = ErasureCoding::new();
    let mut archiver = Archiver::new(erasure_coding);
    let first_block = vec![0u8; RecordedHistorySegment::SIZE];
    let block_1_outcome = archiver.add_block(first_block.clone(), Vec::new()).unwrap();
    let archived_segments = block_1_outcome.archived_segments;
    let object_mapping = block_1_outcome.global_objects;
    assert_eq!(archived_segments.len(), 1);
    assert_eq!(object_mapping.len(), 0);

    let archived_segment = archived_segments.into_iter().next().unwrap();
    let left_unarchived_from_first_block = first_block.len() as u32
        - archived_segment
            .segment_header
            .last_archived_block
            .archived_progress
            .partial()
            .unwrap()
            .get();

    let mut second_block = vec![0u8; RecordedHistorySegment::SIZE * 2];
    let object_mapping = BlockObject {
        hash: Blake3Hash::default(),
        // Offset is designed to fall exactly on the edge of the segment
        offset: RecordedHistorySegment::SIZE as u32
            // Segment header segment item
            - SegmentItem::ParentSegmentHeader(SegmentHeader {
                segment_index: SegmentIndex::ZERO.into(),
                segment_root: Default::default(),
                prev_segment_header_hash: Default::default(),
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::ZERO.into(),
                    archived_progress: ArchivedBlockProgress::new_complete(),
                },
            })
                .encoded_size() as u32
            // Block continuation segment item enum variant
            - 1
            // Block continuation segment item bytes length
            - u32::encoded_fixed_size().unwrap() as u32
            // Block continuation segment item bytes (that didn't fit into the very first segment)
            - left_unarchived_from_first_block
            // One byte for block start segment item enum variant
            - 1
            // Bytes length
            - u32::encoded_fixed_size().unwrap() as u32,
    };
    let mut mapped_bytes = [0u8; 32];
    rng.fill_bytes(&mut mapped_bytes);
    let mapped_bytes = mapped_bytes.to_vec().encode();
    // Write mapped bytes at expected offset in source data
    second_block[object_mapping.offset as usize..][..mapped_bytes.len()]
        .copy_from_slice(&mapped_bytes);

    // First ensure that any smaller offset will get translated into the first archived segment,
    // this is a protection against code regressions
    {
        let block_2_outcome = archiver
            .clone()
            .add_block(
                second_block.clone(),
                vec![BlockObject {
                    hash: object_mapping.hash,
                    offset: object_mapping.offset - 1,
                }],
            )
            .unwrap();
        let archived_segments = block_2_outcome.archived_segments;
        let object_mapping = block_2_outcome.global_objects;

        assert_eq!(archived_segments.len(), 2);
        assert_eq!(object_mapping.len(), 1);
        assert_eq!(
            object_mapping[0].piece_index.segment_index(),
            archived_segments[0].segment_header.segment_index(),
        );
    }

    let block_2_outcome = archiver
        .add_block(second_block, vec![object_mapping])
        .unwrap();
    let archived_segments = block_2_outcome.archived_segments;
    let object_mapping = block_2_outcome.global_objects;

    assert_eq!(archived_segments.len(), 2);
    // Object should fall in the next archived segment
    assert_eq!(object_mapping.len(), 1);
    assert_eq!(
        object_mapping[0].piece_index.segment_index(),
        archived_segments[1].segment_header.segment_index(),
    );

    // Ensure bytes are mapped correctly
    assert_eq!(
        archived_segments[1].pieces[0].record().as_flattened()[object_mapping[0].offset as usize..]
            [..mapped_bytes.len()],
        mapped_bytes
    );
}
