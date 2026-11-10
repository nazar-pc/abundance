use crate::page_group::temporary::super_segment_headers::StorageItemTemporarySuperSegmentHeaders;
use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::{super_segment_header, write_to_aligned_pages};
use ab_core_primitives::segments::SuperSegmentHeader;
use std::assert_matches;

// Exhaustive checks of a large number of headers take too long under Miri
const NUM_SUPER_SEGMENT_HEADERS: &[u64] = if cfg!(miri) {
    &[0, 1, 2, 3]
} else {
    &[0, 1, 2, 3, 50]
};

fn storage_item(num_super_segment_headers: u64) -> StorageItemTemporarySuperSegmentHeaders {
    StorageItemTemporarySuperSegmentHeaders {
        super_segment_headers: (0..num_super_segment_headers)
            .map(super_segment_header)
            .collect(),
    }
}

#[test]
fn write_needs_total_bytes() {
    for &num_super_segment_headers in NUM_SUPER_SEGMENT_HEADERS {
        let storage_item = storage_item(num_super_segment_headers);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;

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
fn read_returns_written_super_segment_headers() {
    for &num_super_segment_headers in NUM_SUPER_SEGMENT_HEADERS {
        let storage_item = storage_item(num_super_segment_headers);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let buffer_len = total_bytes + size_of::<SuperSegmentHeader>() + 1;
        let (result, pages) =
            write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        // The caller frames storage items, so trailing bytes are ignored
        for read_len in [total_bytes, buffer_len] {
            let read_storage_item =
                StorageItemTemporarySuperSegmentHeaders::read(&bytes[..read_len]).unwrap();
            assert_eq!(
                read_storage_item.super_segment_headers,
                storage_item.super_segment_headers
            );
        }
    }
}

#[test]
fn read_of_truncated_bytes_needs_more_bytes() {
    for &num_super_segment_headers in NUM_SUPER_SEGMENT_HEADERS {
        let storage_item = storage_item(num_super_segment_headers);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let (result, pages) =
            write_to_aligned_pages(total_bytes, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        for read_len in 0..total_bytes {
            // Missing bytes of the `u32` count prefix or of the super segment header being read
            let expected_missing_bytes = match read_len.checked_sub(size_of::<u32>()) {
                Some(headers_len) => {
                    size_of::<SuperSegmentHeader>() - headers_len % size_of::<SuperSegmentHeader>()
                }
                None => size_of::<u32>() - read_len,
            };

            let result = StorageItemTemporarySuperSegmentHeaders::read(&bytes[..read_len]);
            let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
                panic!("Unexpected result for read length {read_len}: {result:?}");
            };
            assert_eq!(missing_bytes, expected_missing_bytes);
        }
    }
}

/// The number of super segment headers isn't trusted before the bytes are checked to hold that
/// many, a corrupted one doesn't allocate memory for them
#[test]
fn read_of_more_super_segment_headers_than_bytes_needs_more_bytes() {
    let (result, pages) = write_to_aligned_pages(size_of::<u32>(), |buffer| {
        buffer.write_copy_of_slice(&u32::MAX.to_le_bytes());
        Ok(buffer.len())
    });
    result.unwrap();
    let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

    let result = StorageItemTemporarySuperSegmentHeaders::read(&bytes[..size_of::<u32>()]);
    let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(missing_bytes, size_of::<SuperSegmentHeader>());
}

/// Alignment is checked before the length, so misaligned bytes are rejected even when they are
/// truncated or hold no super segment headers
#[test]
fn read_of_misaligned_bytes_fails() {
    for &num_super_segment_headers in NUM_SUPER_SEGMENT_HEADERS {
        let storage_item = storage_item(num_super_segment_headers);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let (result, pages) = write_to_aligned_pages(1 + total_bytes, |buffer| {
            storage_item.write(&mut buffer[1..])
        });
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        for read_len in size_of::<u32>()..=total_bytes {
            let result = StorageItemTemporarySuperSegmentHeaders::read(&bytes[1..][..read_len]);
            assert_matches!(
                result,
                Err(StorageItemError::InvalidDataAlignment {
                    data_type: "SuperSegmentHeader"
                }),
                "Unexpected result for read length {read_len}"
            );
        }
    }
}
