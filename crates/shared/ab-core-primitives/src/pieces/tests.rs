use crate::pieces::{ExtendedRecord, Record, RecordChunk};
use crate::sectors::SBucket;
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
