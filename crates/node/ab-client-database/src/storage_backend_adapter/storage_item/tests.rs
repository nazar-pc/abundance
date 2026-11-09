use crate::page_group::temporary::StorageItemTemporary;
use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::StorageBackendAdapter;
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemContainer, StorageItemError,
};
use crate::test_utils::{MemoryStorageBackend, segment_header};
use ab_core_primitives::segments::SegmentHeader;

#[test]
fn read_of_truncated_storage_item_needs_more_bytes() {
    // Enough segment headers to not fit into the first page
    let num_segment_headers = AlignedPage::SIZE / size_of::<SegmentHeader>() + 1;
    let container = StorageItemContainer {
        sequence_number: 0,
        storage_item: StorageItemTemporary::SegmentHeaders(StorageItemTemporarySegmentHeaders {
            segment_headers: (0..)
                .map(segment_header)
                .take(num_segment_headers)
                .collect(),
        }),
    };
    let mut pages = Vec::new();
    StorageBackendAdapter::<MemoryStorageBackend>::write_pages_to_buffer(
        &container, None, &mut pages, 0,
    )
    .unwrap();
    let storage_item_bytes_in_first_page = AlignedPage::SIZE
        - StorageItemContainer::<StorageItemTemporary>::prefix_size()
            .next_multiple_of(size_of::<u128>());

    let result = StorageItemContainer::<StorageItemTemporary>::read_from_pages(&pages[..1]);
    let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(
        missing_bytes,
        container.storage_item.total_bytes() - storage_item_bytes_in_first_page
    );
}
