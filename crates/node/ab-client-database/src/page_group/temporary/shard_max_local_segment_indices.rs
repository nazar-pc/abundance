#[cfg(test)]
mod tests;

use crate::storage_backend::OwnedPages;
use crate::storage_backend_adapter::storage_item::{StorageItemError, min_segment_item_overhead};
use ab_core_primitives::segments::LocalSegmentIndex;
use ab_core_primitives::shard::ShardIndex;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use std::mem::MaybeUninit;

/// The highest local segment index of a shard
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(C)]
pub(crate) struct ShardMaxLocalSegmentIndex {
    pub(crate) shard_index: ShardIndex,
    pub(crate) local_segment_index: Unaligned<LocalSegmentIndex>,
}

/// The highest local segment indices of a subset of shards
#[derive(Debug)]
pub(crate) struct StorageItemTemporaryShardMaxLocalSegmentIndices {
    pub(crate) shard_max_local_segment_indices: Vec<ShardMaxLocalSegmentIndex>,
}

impl StorageItemTemporaryShardMaxLocalSegmentIndices {
    pub(super) fn total_bytes(&self) -> Result<u32, StorageItemError> {
        u32::try_from(size_of_val(self.shard_max_local_segment_indices.as_slice()))
            .ok()
            .and_then(|entries_len| entries_len.checked_add(Self::prefix_size() as u32))
            .ok_or(StorageItemError::StorageItemTooLarge)
    }

    /// The number of entries that can be added without increasing the number of pages this storage
    /// item occupies
    pub(crate) fn num_free_entries_in_last_page(&self) -> usize {
        let num_free_bytes = self
            .shard_max_local_segment_indices
            .len()
            .checked_mul(size_of::<ShardMaxLocalSegmentIndex>())
            .and_then(|entries_len| {
                entries_len.checked_add(Self::prefix_size() + min_segment_item_overhead())
            })
            .and_then(|num_bytes| {
                let num_occupied_bytes =
                    num_bytes.checked_next_multiple_of(OwnedPages::ALIGN as usize)?;
                num_occupied_bytes.checked_sub(num_bytes)
            })
            .expect("Entries of at most `ShardIndex::MAX_SHARDS` shards take 12 MiB; qed");

        num_free_bytes / size_of::<ShardMaxLocalSegmentIndex>()
    }

    const fn prefix_size() -> usize {
        const PREFIX_SIZE: usize = size_of::<u32>();
        const {
            // Ensure that writing entries after `u32` length prefix will result in correctly
            // aligned data
            assert!(align_of::<ShardMaxLocalSegmentIndex>() == PREFIX_SIZE);
        }
        PREFIX_SIZE
    }

    pub(super) fn write(
        &self,
        mut buffer: &mut [MaybeUninit<u8>],
    ) -> Result<usize, StorageItemError> {
        let buffer_len = buffer.len();
        let total_bytes = self.total_bytes()? as usize;

        if buffer_len < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer_len,
            });
        }

        // Write the number of entries
        {
            let num_entries = buffer
                .split_off_mut(..Self::prefix_size())
                .expect("Total length checked above; qed");

            num_entries.write_copy_of_slice(
                &u32::try_from(self.shard_max_local_segment_indices.len())
                    .expect(
                        "Each entry takes at least one byte out of `Self::total_bytes()`, which \
                        is `u32`; qed",
                    )
                    .to_le_bytes(),
            );
        }

        buffer
            .split_off_mut(..size_of_val(self.shard_max_local_segment_indices.as_slice()))
            .expect("Total length checked above; qed")
            .write_copy_of_slice(ShardMaxLocalSegmentIndex::slice_as_bytes(
                &self.shard_max_local_segment_indices,
            ));

        Ok(total_bytes)
    }

    /// Read the storage item from `buffer`.
    ///
    /// The layout is as follows:
    /// * number of entries: u32 as aligned little-endian bytes
    /// * entries concatenated, each is:
    ///   * shard index: u32 as aligned little-endian bytes
    ///   * max local segment index of the shard: u64 as unaligned little-endian bytes
    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes;
        (prefix_bytes, buffer) = buffer
            .split_first_chunk::<{ Self::prefix_size() }>()
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::prefix_size() - buffer_len))?;

        // Read the number of entries
        let num_entries = u32::from_le_bytes(*prefix_bytes) as usize;

        let Some((entries, _remainder)) =
            ShardMaxLocalSegmentIndex::slice_from_bytes(buffer, num_entries)
        else {
            if !ShardMaxLocalSegmentIndex::ALIGNMENT.is_slice_aligned(buffer) {
                return Err(StorageItemError::InvalidDataAlignment {
                    data_type: "ShardMaxLocalSegmentIndex",
                });
            }
            // Missing bytes of the first entry that doesn't fit
            return Err(StorageItemError::NeedMoreBytes(
                size_of::<ShardMaxLocalSegmentIndex>()
                    - buffer.len() % size_of::<ShardMaxLocalSegmentIndex>(),
            ));
        };

        Ok(Self {
            shard_max_local_segment_indices: entries.to_vec(),
        })
    }
}
