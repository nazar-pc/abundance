use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemError, StorageItemWriteResult,
};
use std::mem::MaybeUninit;

#[derive(Debug)]
pub(crate) enum StorageItemPermanent {
    // TODO
}

impl StorageItem for StorageItemPermanent {
    fn total_bytes(&self) -> usize {
        unreachable!()
    }

    fn write<'a>(
        &self,
        _buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError> {
        unreachable!()
    }

    fn read(variant: u8, _buffer: &[u8]) -> Result<Self, StorageItemError> {
        Err(StorageItemError::UnknownStorageItemVariant(variant))
    }
}
