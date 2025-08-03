use crate::storage_item::{StorageItemError, StorageItemKind};

#[derive(Debug)]
pub(crate) enum StorageItemPermanentKind {
    // TODO
}

impl StorageItemKind for StorageItemPermanentKind {
    fn total_bytes(&self) -> usize {
        unreachable!()
    }

    fn write(&self, _buffer: &mut [u8]) -> Result<(u8, usize), StorageItemError> {
        unreachable!()
    }

    fn read(variant: u8, _buffer: &[u8]) -> Result<Self, StorageItemError> {
        Err(StorageItemError::UnknownStorageItemVariant(variant))
    }
}
