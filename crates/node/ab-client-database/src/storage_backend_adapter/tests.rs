use crate::page_group::temporary::StorageItemTemporary;
use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::storage_backend::{ClientDatabaseStorageBackend, OwnedPages};
use crate::storage_backend_adapter::page_group_header::StorageItemPageGroupHeader;
use crate::storage_backend_adapter::storage_item::StorageItemContainer;
use crate::storage_backend_adapter::{
    PageGroupKind, StorageBackendAdapter, StorageItemHandlerArg, StorageItemHandlers, WriteLocation,
};
use crate::test_utils::{
    HugeStorageBackend, MemoryStorageBackend, database_options, genesis_block_builder,
    open_database, open_database_with_options, segment_header,
};
use crate::{ClientDatabase, ClientDatabaseError, ClientDatabaseOptions, DatabaseId};
use ab_client_api::{ChainInfo, ChainInfoWrite};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::segments::SegmentHeader;
use futures::executor::block_on;
use std::assert_matches;
use std::num::NonZeroU32;

/// Page of `container`
fn page_group_header_page(
    container: &StorageItemContainer<StorageItemPageGroupHeader>,
) -> OwnedPages {
    let mut page = OwnedPages::EMPTY;
    StorageBackendAdapter::<MemoryStorageBackend>::write_pages_to_buffer(
        container,
        None,
        &mut page,
        container.num_pages().unwrap(),
        0,
    )
    .unwrap();

    page
}

/// Replace the page group header at `page_offset` with the one `modify` makes of it, keeping its
/// sequence number
fn modify_page_group_header<Modify>(
    storage_backend: &MemoryStorageBackend,
    page_offset: u32,
    modify: Modify,
) where
    Modify: FnOnce(&mut StorageItemPageGroupHeader),
{
    let (result, page) = block_on(storage_backend.read(OwnedPages::EMPTY, 1, page_offset)).unwrap();
    result.unwrap();
    let (mut container, _num_pages) =
        StorageItemContainer::<StorageItemPageGroupHeader>::read_from_pages(&page.into_shared())
            .unwrap();
    modify(&mut container.storage_item);

    block_on(storage_backend.write(
        page_group_header_page(&container).into_shared(),
        page_offset,
    ))
    .unwrap()
    .0
    .unwrap();
}

#[test]
fn open_rejects_page_group_size_larger_than_storage_backend() {
    // As if the page group header was corrupted or written for a different storage backend
    let page_group_size = NonZeroU32::new(4).unwrap();
    let storage_backend =
        MemoryStorageBackend::with_size(2 * page_group_size.get(), page_group_size);
    let num_pages = storage_backend.num_pages();
    modify_page_group_header(&storage_backend, 0, |page_group_header| {
        page_group_header.page_group_size = num_pages + 1;
    });

    let result = open_database_with_options(database_options(&storage_backend));
    let Err(ClientDatabaseError::PageGroupSizeTooLarge {
        page_group_size,
        num_pages: error_num_pages,
    }) = result
    else {
        panic!("Unexpected result: {:?}", result.err());
    };
    assert_eq!(
        (page_group_size, error_num_pages),
        (num_pages + 1, num_pages)
    );
}

#[test]
fn open_rejects_invalid_first_page_group_header() {
    let open_with_first_page_group_header = |modify: fn(&mut StorageItemPageGroupHeader)| {
        let page_group_size = NonZeroU32::new(4).unwrap();
        let storage_backend =
            MemoryStorageBackend::with_size(2 * page_group_size.get(), page_group_size);
        modify_page_group_header(&storage_backend, 0, modify);
        open_database_with_options(database_options(&storage_backend)).err()
    };
    let version = StorageBackendAdapter::<MemoryStorageBackend>::VERSION;

    assert_matches!(
        open_with_first_page_group_header(|page_group_header| {
            page_group_header.database_version += 1;
        }),
        Some(ClientDatabaseError::UnsupportedDatabaseVersion { database_version })
            if database_version == version + 1
    );
    assert_matches!(
        open_with_first_page_group_header(|page_group_header| {
            page_group_header.page_group_size = 1;
        }),
        Some(ClientDatabaseError::PageGroupSizeTooSmall { page_group_size: 1 })
    );
    assert_matches!(
        open_with_first_page_group_header(|page_group_header| {
            page_group_header.page_group_kind = PageGroupKind::Temporary;
        }),
        Some(ClientDatabaseError::NonPermanentFirstPageGroup)
    );
}

#[test]
fn open_rejects_unformatted_storage() {
    let page_group_size = NonZeroU32::new(4).unwrap();
    let storage_backend =
        MemoryStorageBackend::with_size(2 * page_group_size.get(), page_group_size);
    block_on(storage_backend.write(OwnedPages::zeroed(OwnedPages::ALIGN).into_shared(), 0))
        .unwrap()
        .0
        .unwrap();

    assert_matches!(
        open_database_with_options(database_options(&storage_backend)),
        Err(ClientDatabaseError::Unformatted)
    );
}

#[test]
fn open_rejects_page_group_size_exceeding_buffer_capacity() {
    // Storage backend is large enough, but the page group doesn't fit into a buffer for reading it
    let first_page = page_group_header_page(&StorageItemContainer {
        sequence_number: 0,
        storage_item: StorageItemPageGroupHeader {
            database_id: DatabaseId::new([1; _]),
            database_version: StorageBackendAdapter::<HugeStorageBackend>::VERSION,
            page_group_kind: PageGroupKind::Permanent,
            padding: [0; _],
            page_group_size: u32::MAX,
        },
    });

    let result = block_on(ClientDatabase::<OwnedBeaconChainBlock, _>::open(
        ClientDatabaseOptions {
            block_confirmation_depth: BlockNumber::from(5),
            genesis_block_builder,
            storage_backend: HugeStorageBackend {
                first_page: first_page.into_shared(),
            },
            ..
        },
    ));
    assert_matches!(
        result.err(),
        Some(ClientDatabaseError::PageGroupSizeExceedsBufferCapacity {
            page_group_size: u32::MAX
        })
    );
}

#[test]
fn page_groups_with_foreign_headers_are_free() {
    // Page groups 2, 3 and 4 get a different database ID, database version and page group size
    // than the first page group, as if they were left there by another database
    let page_group_size = NonZeroU32::new(4).unwrap();
    let storage_backend =
        MemoryStorageBackend::with_size(5 * page_group_size.get(), page_group_size);
    let database = open_database(&storage_backend);
    // Three storage items of one page fill a temporary page group
    for local_segment_index in 0..12 {
        block_on(database.persist_segment_headers(vec![segment_header(local_segment_index)]))
            .unwrap();
    }
    drop(database);
    let foreign_page_offsets =
        [2, 3, 4].map(|page_group_index| page_group_index * page_group_size.get());
    let [
        database_id_page_offset,
        database_version_page_offset,
        page_group_size_page_offset,
    ] = foreign_page_offsets;
    modify_page_group_header(
        &storage_backend,
        database_id_page_offset,
        |page_group_header| {
            page_group_header.database_id = DatabaseId::new([1; _]);
        },
    );
    modify_page_group_header(
        &storage_backend,
        database_version_page_offset,
        |page_group_header| {
            page_group_header.database_version += 1;
        },
    );
    modify_page_group_header(
        &storage_backend,
        page_group_size_page_offset,
        |page_group_header| {
            page_group_header.page_group_size *= 2;
        },
    );

    let database = open_database(&storage_backend);

    assert_eq!(database.last_segment_header(), Some(segment_header(2)));
    {
        let state = database.inner.state.read_blocking();
        let storage_backend_adapter = state.storage_backend_adapter.read_blocking();
        assert_eq!(
            storage_backend_adapter.free_page_groups,
            foreign_page_offsets
        );
    }
}

/// Open a storage backend adapter on `storage_backend`, returns it together with the page offsets
/// and segment headers of storage items in the order they were read in
fn open_storage_backend_adapter(
    storage_backend: &MemoryStorageBackend,
) -> (
    StorageBackendAdapter<MemoryStorageBackend>,
    Vec<(u32, Vec<SegmentHeader>)>,
) {
    let mut storage_items = Vec::new();
    let storage_backend_adapter = block_on(StorageBackendAdapter::open(
        0,
        StorageItemHandlers {
            permanent: |_arg| Ok(()),
            temporary: |arg: StorageItemHandlerArg<StorageItemTemporary>| {
                let StorageItemTemporary::SegmentHeaders(storage_item) = arg.storage_item else {
                    panic!("Only segment headers are written");
                };
                storage_items.push((arg.page_offset, storage_item.segment_headers));
                Ok(())
            },
        },
        storage_backend.clone(),
    ))
    .unwrap();

    (storage_backend_adapter, storage_items)
}

/// Write a storage item with `segment_headers` and return its write location
fn write_segment_headers(
    storage_backend_adapter: &mut StorageBackendAdapter<MemoryStorageBackend>,
    segment_headers: Vec<SegmentHeader>,
) -> WriteLocation {
    block_on(
        storage_backend_adapter.write_storage_item(StorageItemTemporary::SegmentHeaders(
            StorageItemTemporarySegmentHeaders { segment_headers },
        )),
    )
    .unwrap()
}

#[test]
fn page_groups_are_read_in_order_of_sequence_numbers_not_page_offsets() {
    // Storage items of one page fill temporary page groups 1, 2 and 3, then page group 1 gets a
    // different database ID, as if it was left there by another database. The next storage item
    // takes page group 1, so the newest page group is in front of older ones (`H` is the page
    // group header):
    //
    // page group 1: H, segment headers 9..153 (3 pages)
    // page group 2: H, segment header 3, segment header 4, segment header 5
    // page group 3: H, segment header 6, segment header 7, segment header 8
    let page_group_size = NonZeroU32::new(4).unwrap();
    let storage_backend =
        MemoryStorageBackend::with_size(5 * page_group_size.get(), page_group_size);
    let (mut storage_backend_adapter, _storage_items) =
        open_storage_backend_adapter(&storage_backend);
    for local_segment_index in 0..9 {
        write_segment_headers(
            &mut storage_backend_adapter,
            vec![segment_header(local_segment_index)],
        );
    }
    drop(storage_backend_adapter);
    modify_page_group_header(
        &storage_backend,
        page_group_size.get(),
        |page_group_header| {
            page_group_header.database_id = DatabaseId::new([1; _]);
        },
    );

    let (mut storage_backend_adapter, storage_items) =
        open_storage_backend_adapter(&storage_backend);
    let mut expected_storage_items = [9, 10, 11, 13, 14, 15]
        .into_iter()
        .zip(3..)
        .map(|(page_offset, local_segment_index)| {
            (page_offset, vec![segment_header(local_segment_index)])
        })
        .collect::<Vec<_>>();
    assert_eq!(storage_items, expected_storage_items);
    // Takes all pages after the page group header, so no storage item of page group 1 is left
    let segment_headers = (9..153).map(segment_header).collect::<Vec<_>>();
    assert_eq!(
        write_segment_headers(&mut storage_backend_adapter, segment_headers.clone()),
        WriteLocation {
            page_offset: 5,
            num_pages: 3,
        }
    );
    drop(storage_backend_adapter);

    let (_storage_backend_adapter, storage_items) = open_storage_backend_adapter(&storage_backend);
    expected_storage_items.push((5, segment_headers));
    assert_eq!(storage_items, expected_storage_items);
}
