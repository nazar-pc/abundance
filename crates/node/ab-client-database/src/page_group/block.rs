#[expect(
    clippy::module_inception,
    reason = "Using the same name on purpose for now"
)]
pub(crate) mod block;
pub(crate) mod segment_headers;

use crate::page_group::block::block::StorageItemBlockBlock;
use crate::page_group::block::segment_headers::StorageItemBlockSegmentHeaders;
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
}

// TODO: Rename to `temporary` or something?
/// Storage items that are produced as the result of blocks being imported
#[derive(Debug)]
pub(crate) enum StorageItemBlock {
    Block(StorageItemBlockBlock),
    SegmentHeaders(StorageItemBlockSegmentHeaders),
}

impl StorageItem for StorageItemBlock {
    #[inline(always)]
    fn total_bytes(&self) -> usize {
        match self {
            Self::Block(block) => block.total_bytes(),
            Self::SegmentHeaders(segment_headers) => segment_headers.total_bytes(),
        }
    }

    #[inline(always)]
    fn write<'a>(
        &self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError> {
        let (variant, storage_item_size) = match self {
            Self::Block(block) => (StorageItemBlockVariant::Block, block.write(buffer)?),
            StorageItemBlock::SegmentHeaders(segment_headers) => (
                StorageItemBlockVariant::SegmentHeaders,
                segment_headers.write(buffer)?,
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
            StorageItemBlockVariant::Block => Self::Block(StorageItemBlockBlock::read(buffer)?),
            StorageItemBlockVariant::SegmentHeaders => {
                Self::SegmentHeaders(StorageItemBlockSegmentHeaders::read(buffer)?)
            }
        })
    }
}

impl UniqueStorageItem for StorageItemBlock {
    #[inline(always)]
    fn page_group_kind() -> PageGroupKind {
        PageGroupKind::Block
    }
}
