use crate::page_group::permanent::StorageItemPermanent;
use crate::page_group::temporary::StorageItemTemporary;
use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::storage_backend::OwnedPages;
use crate::storage_backend_adapter::StorageBackendAdapter;
use crate::storage_backend_adapter::page_group_header::StorageItemPageGroupHeader;
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemContainer, StorageItemError,
};
use crate::test_utils::{MemoryStorageBackend, segment_header};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::segments::SegmentHeader;
use ab_io_type::trivial_type::TrivialType;
use std::assert_matches;
use std::mem::offset_of;

#[test]
fn read_of_truncated_storage_item_needs_more_bytes() {
    // Enough segment headers to not fit into the first page
    let num_segment_headers = OwnedPages::ALIGN as usize / size_of::<SegmentHeader>() + 1;
    let container = StorageItemContainer {
        sequence_number: 0,
        storage_item: StorageItemTemporary::SegmentHeaders(StorageItemTemporarySegmentHeaders {
            segment_headers: (0..)
                .map(segment_header)
                .take(num_segment_headers)
                .collect(),
        }),
    };
    let mut pages = OwnedPages::EMPTY;
    StorageBackendAdapter::<MemoryStorageBackend>::write_pages_to_buffer(
        &container,
        None,
        &mut pages,
        container.num_pages().unwrap(),
        0,
    )
    .unwrap();
    let storage_item_bytes_in_first_page = OwnedPages::ALIGN as usize
        - StorageItemContainer::<StorageItemTemporary>::prefix_size()
            .next_multiple_of(size_of::<u128>());

    let (first_page, _rest) = pages.into_shared().split_at(OwnedPages::ALIGN).unwrap();
    let result = StorageItemContainer::<StorageItemTemporary>::read_from_pages(&first_page);
    let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(
        missing_bytes,
        container.storage_item.total_bytes().unwrap() as usize - storage_item_bytes_in_first_page
    );
}

#[test]
fn write_zeroes_prefix_padding() {
    let container = StorageItemContainer {
        sequence_number: 0,
        storage_item: StorageItemTemporary::SegmentHeaders(StorageItemTemporarySegmentHeaders {
            segment_headers: vec![segment_header(0)],
        }),
    };
    // A page with garbage in it, so that bytes that are not written can be detected
    let mut page = OwnedPages::from_bytes(&[u8::MAX; OwnedPages::ALIGN as usize]);
    page.truncate(0);

    container
        .write_to_pages(&mut page.spare_capacity_mut()[..OwnedPages::ALIGN as usize])
        .unwrap();

    // SAFETY: The page was initialized above
    unsafe {
        page.set_len(OwnedPages::ALIGN);
    }
    let bytes = page.as_slice();
    let prefix_size = StorageItemContainer::<StorageItemTemporary>::prefix_size();
    let prefix_padding = &bytes[prefix_size..prefix_size.next_multiple_of(size_of::<u128>())];
    assert!(!prefix_padding.is_empty());
    assert!(prefix_padding.iter().all(|&byte| byte == 0));
}

#[test]
fn read_of_unknown_storage_item_variant_fails() {
    let buffer = SharedAlignedBuffer::from_bytes(&[0; 16]);

    // Temporary storage items have variants 0 to 2
    for variant in [3, u8::MAX] {
        assert_matches!(
            StorageItemTemporary::read(variant, &buffer),
            Err(StorageItemError::UnknownStorageItemVariant(unknown_variant))
                if unknown_variant == variant
        );
    }
    // There are no permanent storage items yet
    for variant in [0, u8::MAX] {
        assert_matches!(
            StorageItemPermanent::read(variant, &buffer),
            Err(StorageItemError::UnknownStorageItemVariant(unknown_variant))
                if unknown_variant == variant
        );
    }
}

#[test]
fn read_of_invalid_page_group_header_fails() {
    let size = StorageItemPageGroupHeader::SIZE as usize;
    let mut bytes = vec![0; size];
    assert_matches!(
        StorageItemPageGroupHeader::read(0, &SharedAlignedBuffer::from_bytes(&bytes)),
        Ok(_)
    );

    assert_matches!(
        StorageItemPageGroupHeader::read(1, &SharedAlignedBuffer::from_bytes(&bytes)),
        Err(StorageItemError::UnknownStorageItemVariant(1))
    );
    assert_matches!(
        StorageItemPageGroupHeader::read(0, &SharedAlignedBuffer::from_bytes(&bytes[..size - 1])),
        Err(StorageItemError::BufferTooSmall { expected, actual })
            if expected == size && actual == size - 1
    );

    // Page group kinds are 0 and 1
    bytes[offset_of!(StorageItemPageGroupHeader, page_group_kind)] = 2;
    assert_matches!(
        StorageItemPageGroupHeader::read(0, &SharedAlignedBuffer::from_bytes(&bytes)),
        Err(StorageItemError::InvalidBufferContents)
    );
}
