use crate::pieces::{ExtendedRecord, FlatPieces, Piece, Record, RecordChunk};
use crate::sectors::SBucket;
use ab_io_type::trivial_type::TrivialType;
use core::ptr;

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
    assert!(ptr::eq(
        Record::array_from_repr(Record::array_to_repr(records)),
        records
    ));
    let s_buckets = extended_record.as_s_buckets();
    assert!(ptr::eq(
        RecordChunk::array_from_repr(RecordChunk::array_to_repr(s_buckets)),
        s_buckets
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
