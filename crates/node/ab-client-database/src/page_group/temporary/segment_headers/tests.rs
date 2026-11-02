use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::{segment_header, write_to_aligned_pages};
use ab_core_primitives::segments::SegmentHeader;

// Exhaustive checks of a large number of headers take too long under Miri
const NUM_SEGMENT_HEADERS: &[u64] = if cfg!(miri) {
    &[0, 1, 2, 3]
} else {
    &[0, 1, 2, 3, 50]
};

fn storage_item(num_segment_headers: u64) -> StorageItemTemporarySegmentHeaders {
    StorageItemTemporarySegmentHeaders {
        segment_headers: (0..num_segment_headers).map(segment_header).collect(),
    }
}

#[test]
fn write_needs_total_bytes() {
    for &num_segment_headers in NUM_SEGMENT_HEADERS {
        let storage_item = storage_item(num_segment_headers);
        let total_bytes = storage_item.total_bytes();

        for buffer_len in [
            total_bytes,
            total_bytes + 1,
            total_bytes + AlignedPage::SIZE,
        ] {
            let (result, _pages) =
                write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
            assert_eq!(result.unwrap(), total_bytes);
        }

        for buffer_len in 0..total_bytes {
            let (result, _pages) =
                write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
            let Err(StorageItemError::BufferTooSmall { expected, actual }) = result else {
                panic!("Unexpected result for buffer length {buffer_len}: {result:?}");
            };
            assert_eq!((expected, actual), (total_bytes, buffer_len));
        }
    }
}

#[test]
fn read_returns_written_segment_headers() {
    for &num_segment_headers in NUM_SEGMENT_HEADERS {
        let storage_item = storage_item(num_segment_headers);
        let total_bytes = storage_item.total_bytes();
        let buffer_len = total_bytes + size_of::<SegmentHeader>() + 1;
        let (result, pages) =
            write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        // The caller frames storage items, so trailing bytes are ignored
        for read_len in [total_bytes, buffer_len] {
            let read_storage_item =
                StorageItemTemporarySegmentHeaders::read(&bytes[..read_len]).unwrap();
            assert_eq!(
                read_storage_item.segment_headers,
                storage_item.segment_headers
            );
        }
    }
}

#[test]
fn read_of_truncated_bytes_needs_more_bytes() {
    for &num_segment_headers in NUM_SEGMENT_HEADERS {
        let storage_item = storage_item(num_segment_headers);
        let total_bytes = storage_item.total_bytes();
        let (result, pages) =
            write_to_aligned_pages(total_bytes, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        for read_len in 0..total_bytes {
            // Missing bytes of the `u32` count prefix or of the segment header being read
            let expected_missing_bytes = match read_len.checked_sub(size_of::<u32>()) {
                Some(headers_len) => {
                    size_of::<SegmentHeader>() - headers_len % size_of::<SegmentHeader>()
                }
                None => size_of::<u32>() - read_len,
            };

            let result = StorageItemTemporarySegmentHeaders::read(&bytes[..read_len]);
            let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
                panic!("Unexpected result for read length {read_len}: {result:?}");
            };
            assert_eq!(missing_bytes, expected_missing_bytes);
        }
    }
}

#[test]
fn read_of_misaligned_bytes_fails() {
    let storage_item = storage_item(1);
    let total_bytes = storage_item.total_bytes();
    let (result, pages) = write_to_aligned_pages(1 + total_bytes, |buffer| {
        storage_item.write(&mut buffer[1..])
    });
    result.unwrap();
    let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

    let result = StorageItemTemporarySegmentHeaders::read(&bytes[1..][..total_bytes]);
    assert!(
        matches!(
            result,
            Err(StorageItemError::InvalidDataAlignment {
                data_type: "SegmentHeader"
            })
        ),
        "{result:?}"
    );
}
