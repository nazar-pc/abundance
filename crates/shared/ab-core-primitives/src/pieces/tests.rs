use crate::block::BlockNumber;
use crate::hashes::Blake3Hash;
use crate::pieces::{
    ExtendedRecord, FlatPieces, InnerPiece, Piece, PieceHeader, PieceIndex, PiecePosition, Record,
    RecordChunk, RecordChunksRoot, RecordProof,
};
use crate::sectors::SBucket;
use crate::segments::{
    LocalSegmentIndex, RecordedHistorySegment, SegmentIndex, SegmentPosition, SegmentRoot,
    ShardSegmentRootWithPosition, SuperSegment, SuperSegmentHeader, SuperSegmentIndex,
    SuperSegmentRoot,
};
use crate::shard::ShardIndex;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::blake3::BalancedMerkleTree;
use core::{array, ptr};

#[test]
fn extended_record_s_buckets() {
    let mut extended_record = ExtendedRecord::new_boxed();

    // Only chunks at the boundaries, other chunks would make it too slow under Miri
    let s_buckets = extended_record.as_s_buckets_mut();
    s_buckets[0] = RecordChunk::from([1; _]);
    s_buckets[Record::NUM_CHUNKS - 1] = RecordChunk::from([2; _]);
    s_buckets[Record::NUM_CHUNKS] = RecordChunk::from([3; _]);
    s_buckets[Record::NUM_S_BUCKETS - 1] = RecordChunk::from([4; _]);

    // Source chunks are followed by parity chunks
    assert_eq!(extended_record.source[0], [1; _]);
    assert_eq!(extended_record.source[Record::NUM_CHUNKS - 1], [2; _]);
    assert_eq!(extended_record.parity[0], [3; _]);
    assert_eq!(extended_record.parity[Record::NUM_CHUNKS - 1], [4; _]);

    // The same chunks through indexing
    assert_eq!(extended_record[SBucket::ZERO], RecordChunk::from([1; _]));
    assert_eq!(extended_record[SBucket::MAX], RecordChunk::from([4; _]));
    extended_record[SBucket::MAX] = RecordChunk::from([5; _]);
    assert_eq!(extended_record.parity[Record::NUM_CHUNKS - 1], [5; _]);

    let records = extended_record.as_records();
    assert!(ptr::eq(
        &raw const records[0],
        &raw const extended_record.source
    ));
    assert!(ptr::eq(
        &raw const records[1],
        &raw const extended_record.parity
    ));
}

#[test]
fn flat_pieces() {
    let mut flat_pieces = FlatPieces::new(3);
    assert_eq!(flat_pieces.len(), 3);
    for (index, piece) in flat_pieces.iter_mut().enumerate() {
        piece.record[0] = [index as u8 + 1; _];
    }

    // Owned pieces are copied
    for (index, piece) in flat_pieces.pieces().enumerate() {
        assert_eq!(piece.record[0], [index as u8 + 1; _]);
        assert!(!ptr::eq(
            piece.as_ref().as_ptr(),
            flat_pieces[index].as_bytes().as_ptr()
        ));
    }

    // Shared pieces reference the same allocation
    let mut flat_pieces = flat_pieces.to_shared();
    let pieces = flat_pieces.pieces().collect::<Vec<Piece>>();
    assert_eq!(pieces.len(), 3);
    for (index, piece) in pieces.iter().enumerate() {
        assert_eq!(piece.record[0], [index as u8 + 1; _]);
        assert!(ptr::eq(
            piece.as_ref().as_ptr(),
            flat_pieces[index].as_bytes().as_ptr()
        ));
    }
    assert_eq!(flat_pieces.clone(), flat_pieces);

    // Modification copies shared pieces first, pieces that were already created are not modified
    flat_pieces[0].record[0] = [4; _];
    assert_eq!(flat_pieces[0].record[0], [4; _]);
    assert_eq!(pieces[0].record[0], [1; _]);
    assert!(!ptr::eq(
        pieces[0].as_ref().as_ptr(),
        flat_pieces[0].as_bytes().as_ptr()
    ));
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Derives the record root of a full record, too slow under Miri"
)]
fn piece_is_valid() {
    let piece_position = PiecePosition::from(100);
    let mut record_piece = InnerPiece::new_boxed();
    record_piece.record[0] = [1; _];
    record_piece.header.parity_chunks_root = RecordChunksRoot::from([2; _]);
    let record_root = *record_piece.record_root();

    // The same record at the same position in segments of different shards, the rest of the
    // record roots are arbitrary
    let segments = [ShardIndex::BEACON_CHAIN, ShardIndex::new(1).unwrap()]
        .into_iter()
        .zip(0..)
        .map(|(shard_index, segment_position)| {
            let record_roots =
                array::from_fn::<_, { RecordedHistorySegment::NUM_PIECES }, _>(|position| {
                    if position == usize::from(piece_position) {
                        record_root
                    } else {
                        [position as u8; _]
                    }
                });
            let merkle_tree = BalancedMerkleTree::new(&record_roots);
            let record_proof = merkle_tree
                .all_proofs()
                .nth(usize::from(piece_position))
                .unwrap();

            let shard_segment_root = ShardSegmentRootWithPosition {
                shard_index,
                segment_position: SegmentPosition::from(segment_position),
                local_segment_index: LocalSegmentIndex::from(5),
                segment_root: SegmentRoot::from(merkle_tree.root()),
            };
            (shard_segment_root, RecordProof::from(record_proof))
        })
        .collect::<Vec<_>>();

    let previous_header = SuperSegmentHeader {
        index: SuperSegmentIndex::ONE.into(),
        root: SuperSegmentRoot::default(),
        prev_super_segment_header_hash: Blake3Hash::default(),
        max_segment_index: SegmentIndex::from(9).into(),
        target_beacon_chain_block_number: BlockNumber::ONE.into(),
        num_segments: 1,
    };
    let super_segment = SuperSegment::new(
        &previous_header,
        BlockNumber::from(10),
        segments
            .iter()
            .map(|(shard_segment_root, _record_proof)| *shard_segment_root)
            .collect(),
    )
    .unwrap();
    let header = super_segment.header;

    let pieces = segments
        .iter()
        .map(|(shard_segment_root, record_proof)| {
            let mut piece = record_piece.clone();
            piece.header = PieceHeader {
                shard_index: shard_segment_root.shard_index.into(),
                local_segment_index: shard_segment_root.local_segment_index.into(),
                segment_root: shard_segment_root.segment_root,
                segment_proof: super_segment
                    .proof_for_segment(shard_segment_root.segment_position)
                    .unwrap(),
                parity_chunks_root: record_piece.header.parity_chunks_root,
                record_proof: *record_proof,
            };
            piece
        })
        .collect::<Vec<_>>();
    // Segments 10 and 11
    let piece_indices = [10, 11].map(|segment_index| {
        SegmentIndex::from(segment_index).first_piece_index()
            + PieceIndex::from(u64::from(piece_position))
    });

    let is_valid = |piece: &InnerPiece, header: &SuperSegmentHeader, piece_index| {
        piece.is_valid(
            &header.root,
            header.num_segments,
            header.max_segment_index.as_inner(),
            piece_index,
        )
    };

    for (piece, piece_index, other_piece_index) in [
        (&pieces[0], piece_indices[0], piece_indices[1]),
        (&pieces[1], piece_indices[1], piece_indices[0]),
    ] {
        assert!(is_valid(piece, &header, piece_index));
        // A piece at the same position, but in a different segment
        assert!(!is_valid(piece, &header, other_piece_index));
        // At a different position in the same segment
        assert!(!is_valid(piece, &header, piece_index + PieceIndex::ONE));
        // In a segment before this super segment
        assert!(!is_valid(
            piece,
            &header,
            piece_index - SegmentIndex::from(2).first_piece_index()
        ));
        // Super segments that don't contain the segment
        for other_header in [
            previous_header,
            SuperSegmentHeader {
                num_segments: 0,
                ..header
            },
        ] {
            assert!(!is_valid(piece, &other_header, piece_index));
        }
    }
}
