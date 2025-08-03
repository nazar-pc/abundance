use crate::storage_backend_adapter::storage_item::{StorageItem, StorageItemError};

#[derive(Debug)]
pub(crate) enum StorageItemPermanent {
    // TODO
}

impl StorageItem for StorageItemPermanent {
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
