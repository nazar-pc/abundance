pub(crate) mod block;
pub(crate) mod segment_headers;
pub(crate) mod super_segment_headers;

use crate::page_group::temporary::block::StorageItemTemporaryBlock;
use crate::page_group::temporary::segment_headers::StorageItemTemporarySegmentHeaders;
use crate::page_group::temporary::super_segment_headers::StorageItemTemporarySuperSegmentHeaders;
use crate::storage_backend_adapter::PageGroupKind;
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemError, StorageItemWriteResult, UniqueStorageItem,
};
use std::mem::MaybeUninit;
use strum::FromRepr;

#[derive(Debug, FromRepr)]
#[repr(u8)]
enum StorageItemBlockVariant {
    Block = 0,
    SegmentHeaders = 1,
    SuperSegmentHeaders = 2,
}

/// Temporary storage items that will be pruned from the database eventually
#[derive(Debug)]
pub(crate) enum StorageItemTemporary {
    Block(StorageItemTemporaryBlock),
    SegmentHeaders(StorageItemTemporarySegmentHeaders),
    SuperSegmentHeaders(StorageItemTemporarySuperSegmentHeaders),
}

impl StorageItem for StorageItemTemporary {
    #[inline(always)]
    fn total_bytes(&self) -> usize {
        match self {
            Self::Block(block) => block.total_bytes(),
            Self::SegmentHeaders(segment_headers) => segment_headers.total_bytes(),
            Self::SuperSegmentHeaders(super_segment_headers) => super_segment_headers.total_bytes(),
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
    fn read(variant: u8, buffer: &[u8]) -> Result<Self, StorageItemError> {
        let variant = StorageItemBlockVariant::from_repr(variant)
            .ok_or(StorageItemError::UnknownStorageItemVariant(variant))?;

        Ok(match variant {
            StorageItemBlockVariant::Block => Self::Block(StorageItemTemporaryBlock::read(buffer)?),
            StorageItemBlockVariant::SegmentHeaders => {
                Self::SegmentHeaders(StorageItemTemporarySegmentHeaders::read(buffer)?)
            }
            StorageItemBlockVariant::SuperSegmentHeaders => {
                Self::SuperSegmentHeaders(StorageItemTemporarySuperSegmentHeaders::read(buffer)?)
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
