use crate::storage_backend::ClientDatabaseStorageBackend;
use crate::storage_backend_adapter::page_group_header::StorageItemPageGroupHeader;
use crate::storage_backend_adapter::storage_item::StorageItemContainer;
use crate::storage_backend_adapter::{PageGroupKind, StorageBackendAdapter};
use crate::test_utils::{MemoryStorageBackend, database_options, open_database_with_options};
use crate::{ClientDatabaseError, DatabaseId};
use futures::executor::block_on;

#[test]
fn open_rejects_page_group_size_larger_than_storage_backend() {
    let storage_backend = MemoryStorageBackend::new();
    let num_pages = storage_backend.num_pages();

    // Valid page group header, but with a page group size that doesn't fit into the storage
    // backend, as if it was corrupted or written for a different storage backend
    let container = StorageItemContainer {
        sequence_number: 0,
        storage_item: StorageItemPageGroupHeader {
            database_id: DatabaseId::new([1; _]),
            database_version: StorageBackendAdapter::<MemoryStorageBackend>::VERSION,
            page_group_kind: PageGroupKind::Permanent,
            padding: [0; _],
            page_group_size: u32::MAX,
        },
    };
    let mut buffer = Vec::new();
    StorageBackendAdapter::<MemoryStorageBackend>::write_pages_to_buffer(
        &container,
        None,
        &mut buffer,
        container.num_pages().unwrap(),
        0,
    )
    .unwrap();
    block_on(storage_backend.write(buffer, 0)).unwrap().unwrap();

    let result = open_database_with_options(database_options(&storage_backend));
    let Err(ClientDatabaseError::PageGroupSizeTooLarge {
        page_group_size,
        num_pages: error_num_pages,
    }) = result
    else {
        panic!("Unexpected result: {:?}", result.err());
    };
    assert_eq!((page_group_size, error_num_pages), (u32::MAX, num_pages));
}
