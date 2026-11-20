use crate::page_group::temporary::shard_max_local_segment_indices::{
    ShardMaxLocalSegmentIndex, StorageItemTemporaryShardMaxLocalSegmentIndices,
};
use crate::storage_backend::OwnedPages;
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::write_to_aligned_pages;
use ab_core_primitives::segments::LocalSegmentIndex;
use ab_core_primitives::shard::ShardIndex;
use ab_io_type::unaligned::Unaligned;
use std::assert_matches;

// Exhaustive checks of a large number of entries take too long under Miri
const NUM_ENTRIES: &[u32] = if cfg!(miri) {
    &[0, 1, 2, 3]
} else {
    &[0, 1, 2, 3, 50]
};

fn storage_item(num_entries: u32) -> StorageItemTemporaryShardMaxLocalSegmentIndices {
    StorageItemTemporaryShardMaxLocalSegmentIndices {
        shard_max_local_segment_indices: (0..num_entries)
            .map(|shard_index| ShardMaxLocalSegmentIndex {
                shard_index: ShardIndex::new(shard_index).unwrap(),
                local_segment_index: Unaligned::from(LocalSegmentIndex::from(
                    u64::from(shard_index) * 3,
                )),
            })
            .collect(),
    }
}

#[test]
fn write_needs_total_bytes() {
    for &num_entries in NUM_ENTRIES {
        let storage_item = storage_item(num_entries);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;

        for buffer_len in [
            total_bytes,
            total_bytes + 1,
            total_bytes + OwnedPages::ALIGN as usize,
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
fn write_follows_documented_layout() {
    let storage_item = StorageItemTemporaryShardMaxLocalSegmentIndices {
        shard_max_local_segment_indices: vec![
            ShardMaxLocalSegmentIndex {
                shard_index: ShardIndex::new(3).unwrap(),
                local_segment_index: Unaligned::from(LocalSegmentIndex::from(0x0102)),
            },
            ShardMaxLocalSegmentIndex {
                shard_index: ShardIndex::BEACON_CHAIN,
                local_segment_index: Unaligned::from(LocalSegmentIndex::from(7)),
            },
        ],
    };
    let total_bytes = storage_item.total_bytes().unwrap() as usize;
    let (result, pages) = write_to_aligned_pages(total_bytes, |buffer| storage_item.write(buffer));
    result.unwrap();

    assert_eq!(
        &pages.as_slice()[..total_bytes],
        [
            [2, 0, 0, 0].as_slice(),
            &[3, 0, 0, 0],
            &[2, 1, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0],
            &[7, 0, 0, 0, 0, 0, 0, 0],
        ]
        .concat()
    );
}

#[test]
fn read_returns_written_entries() {
    for &num_entries in NUM_ENTRIES {
        let storage_item = storage_item(num_entries);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let buffer_len = total_bytes + size_of::<ShardMaxLocalSegmentIndex>() + 1;
        let (result, pages) =
            write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = pages.as_slice();

        // The caller frames storage items, so trailing bytes are ignored
        for read_len in [total_bytes, buffer_len] {
            let read_storage_item =
                StorageItemTemporaryShardMaxLocalSegmentIndices::read(&bytes[..read_len]).unwrap();
            assert_eq!(
                read_storage_item.shard_max_local_segment_indices,
                storage_item.shard_max_local_segment_indices
            );
        }
    }
}

#[test]
fn read_of_truncated_bytes_needs_more_bytes() {
    for &num_entries in NUM_ENTRIES {
        let storage_item = storage_item(num_entries);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let (result, pages) =
            write_to_aligned_pages(total_bytes, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = pages.as_slice();

        for read_len in 0..total_bytes {
            // Missing bytes of the `u32` count prefix or of the entry being read
            let expected_missing_bytes = match read_len.checked_sub(size_of::<u32>()) {
                Some(entries_len) => {
                    size_of::<ShardMaxLocalSegmentIndex>()
                        - entries_len % size_of::<ShardMaxLocalSegmentIndex>()
                }
                None => size_of::<u32>() - read_len,
            };

            let result = StorageItemTemporaryShardMaxLocalSegmentIndices::read(&bytes[..read_len]);
            let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
                panic!("Unexpected result for read length {read_len}: {result:?}");
            };
            assert_eq!(missing_bytes, expected_missing_bytes);
        }
    }
}

/// The number of entries isn't trusted before the bytes are checked to hold that many, a corrupted
/// one doesn't allocate memory for them
#[test]
fn read_of_more_entries_than_bytes_needs_more_bytes() {
    let (result, pages) = write_to_aligned_pages(size_of::<u32>(), |buffer| {
        buffer.write_copy_of_slice(&u32::MAX.to_le_bytes());
        Ok(buffer.len())
    });
    result.unwrap();
    let bytes = pages.as_slice();

    let result = StorageItemTemporaryShardMaxLocalSegmentIndices::read(&bytes[..size_of::<u32>()]);
    let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(missing_bytes, size_of::<ShardMaxLocalSegmentIndex>());
}

/// Alignment is checked before the length, so misaligned bytes are rejected even when they are
/// truncated or hold no entries
#[test]
fn read_of_misaligned_bytes_fails() {
    for &num_entries in NUM_ENTRIES {
        let storage_item = storage_item(num_entries);
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let (result, pages) = write_to_aligned_pages(1 + total_bytes, |buffer| {
            storage_item.write(&mut buffer[1..])
        });
        result.unwrap();
        let bytes = pages.as_slice();

        for read_len in size_of::<u32>()..=total_bytes {
            let result =
                StorageItemTemporaryShardMaxLocalSegmentIndices::read(&bytes[1..][..read_len]);
            assert_matches!(
                result,
                Err(StorageItemError::InvalidDataAlignment {
                    data_type: "ShardMaxLocalSegmentIndex"
                }),
                "Unexpected result for read length {read_len}"
            );
        }
    }
}
