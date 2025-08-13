#[expect(
    clippy::module_inception,
    reason = "Using the same name on purpose for now"
)]
pub(crate) mod block;

use crate::page_group::block::block::StorageItemBlockBlock;
use crate::storage_backend_adapter::PageGroupKind;
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemError, StorageItemWriteResult, UniqueStorageItem,
};
use std::mem;
use std::mem::MaybeUninit;
use strum::FromRepr;

#[derive(Debug, FromRepr)]
#[repr(u8)]
enum StorageItemBlockVariant {
    Block = 0,
}

#[derive(Debug)]
pub(crate) enum StorageItemBlock {
    Block(StorageItemBlockBlock),
}

impl StorageItem for StorageItemBlock {
    #[inline(always)]
    fn total_bytes(&self) -> usize {
        match self {
            Self::Block(block) => block.total_bytes(),
        }
    }

    #[inline(always)]
    fn write<'a>(
        &self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError> {
        let (variant, storage_item_size) = match self {
            Self::Block(block) => (StorageItemBlockVariant::Block, block.write(buffer)?),
        };

        let (storage_item_bytes, buffer) = buffer.split_at_mut(storage_item_size);
        // SAFETY: Storage item bytes were just written to
        let storage_item_bytes =
            unsafe { mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(storage_item_bytes) };

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
        })
    }
}

impl UniqueStorageItem for StorageItemBlock {
    #[inline(always)]
    fn page_group_kind() -> PageGroupKind {
        PageGroupKind::Block
    }
}
