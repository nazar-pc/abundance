pub(crate) mod block;
pub(crate) mod segment_headers;
pub(crate) mod shard_max_local_segment_indices;
pub(crate) mod super_segment_headers;

use crate::page_group::temporary::block::StorageItemTemporaryBlock;
use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::page_group::temporary::shard_max_local_segment_indices::StorageItemTemporaryShardMaxLocalSegmentIndices;
use crate::page_group::temporary::super_segment_headers::StorageItemTemporarySuperSegmentHeaders;
use crate::storage_backend_adapter::PageGroupKind;
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemError, StorageItemWriteResult, UniqueStorageItem,
};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align16;
use ab_io_type::trivial_type::TrivialType;
use std::mem::MaybeUninit;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum StorageItemBlockVariant {
    Block = 0,
    SegmentHeaders = 1,
    SuperSegmentHeaders = 2,
    ShardMaxLocalSegmentIndices = 3,
}

/// Temporary storage items that will be pruned from the database eventually
#[derive(Debug)]
pub(crate) enum StorageItemTemporary {
    Block(StorageItemTemporaryBlock),
    SegmentHeaders(StorageItemTemporarySegmentHeaders),
    SuperSegmentHeaders(StorageItemTemporarySuperSegmentHeaders),
    ShardMaxLocalSegmentIndices(StorageItemTemporaryShardMaxLocalSegmentIndices),
}

impl StorageItem for StorageItemTemporary {
    #[inline(always)]
    fn total_bytes(&self) -> Result<u32, StorageItemError> {
        match self {
            Self::Block(block) => block.total_bytes(),
            Self::SegmentHeaders(segment_headers) => segment_headers.total_bytes(),
            Self::SuperSegmentHeaders(super_segment_headers) => super_segment_headers.total_bytes(),
            Self::ShardMaxLocalSegmentIndices(shard_max_local_segment_indices) => {
                shard_max_local_segment_indices.total_bytes()
            }
        }
    }

    #[inline(always)]
    fn write<'a>(
        &self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError> {
        let (variant, storage_item_size) = match self {
            Self::Block(block) => (StorageItemBlockVariant::Block, block.write(buffer)?),
            Self::SegmentHeaders(segment_headers) => (
                StorageItemBlockVariant::SegmentHeaders,
                segment_headers.write(buffer)?,
            ),
            Self::SuperSegmentHeaders(super_segment_headers) => (
                StorageItemBlockVariant::SuperSegmentHeaders,
                super_segment_headers.write(buffer)?,
            ),
            Self::ShardMaxLocalSegmentIndices(shard_max_local_segment_indices) => (
                StorageItemBlockVariant::ShardMaxLocalSegmentIndices,
                shard_max_local_segment_indices.write(buffer)?,
            ),
        };

        let (storage_item_bytes, buffer) = buffer.split_at_mut(storage_item_size);
        // SAFETY: Storage item bytes were just written to
        let storage_item_bytes = unsafe { storage_item_bytes.assume_init_mut() };

        Ok(StorageItemWriteResult {
            storage_item_variant: variant as u8,
            storage_item_bytes,
            buffer,
        })
    }

    #[inline(always)]
    fn read(variant: u8, buffer: &SharedAlignedBuffer<Align16>) -> Result<Self, StorageItemError> {
        let variant = StorageItemBlockVariant::try_from_bytes_exact(&[variant])
            .copied()
            .ok_or(StorageItemError::UnknownStorageItemVariant(variant))?;

        Ok(match variant {
            StorageItemBlockVariant::Block => Self::Block(StorageItemTemporaryBlock::read(buffer)?),
            StorageItemBlockVariant::SegmentHeaders => {
                Self::SegmentHeaders(StorageItemTemporarySegmentHeaders::read(buffer.as_slice())?)
            }
            StorageItemBlockVariant::SuperSegmentHeaders => Self::SuperSegmentHeaders(
                StorageItemTemporarySuperSegmentHeaders::read(buffer.as_slice())?,
            ),
            StorageItemBlockVariant::ShardMaxLocalSegmentIndices => {
                Self::ShardMaxLocalSegmentIndices(
                    StorageItemTemporaryShardMaxLocalSegmentIndices::read(buffer.as_slice())?,
                )
            }
        })
    }
}

impl UniqueStorageItem for StorageItemTemporary {
    #[inline(always)]
    fn page_group_kind() -> PageGroupKind {
        PageGroupKind::Temporary
    }
}
