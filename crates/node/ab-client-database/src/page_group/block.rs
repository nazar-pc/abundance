#[expect(
    clippy::module_inception,
    reason = "Using the same name on purpose for now"
)]
pub(crate) mod block;

use crate::page_group::block::block::StorageItemBlock;
use crate::storage_backend_adapter::storage_item::{StorageItemError, StorageItemKind};
use strum::FromRepr;

#[derive(Debug, FromRepr)]
#[repr(u8)]
enum StorageItemBlockVariant {
    Block = 0,
}

#[derive(Debug)]
pub(crate) enum StorageItemBlockKind {
    Block(StorageItemBlock),
}

impl StorageItemKind for StorageItemBlockKind {
    #[inline(always)]
    fn total_bytes(&self) -> usize {
        match self {
            Self::Block(block) => block.total_bytes(),
        }
    }

    #[inline(always)]
    fn write(&self, buffer: &mut [u8]) -> Result<(u8, usize), StorageItemError> {
        let (variant, storage_item_size) = match self {
            Self::Block(block) => (StorageItemBlockVariant::Block, block.write(buffer)?),
        };

        Ok((variant as u8, storage_item_size))
    }

    #[inline(always)]
    fn read(variant: u8, buffer: &[u8]) -> Result<Self, StorageItemError> {
        let variant = StorageItemBlockVariant::from_repr(variant)
            .ok_or(StorageItemError::UnknownStorageItemVariant(variant))?;

        Ok(match variant {
            StorageItemBlockVariant::Block => Self::Block(StorageItemBlock::read(buffer)?),
        })
    }
}
