use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemError, StorageItemWriteResult,
};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align16;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub(crate) enum StorageItemPermanent {
    // TODO
}

impl StorageItem for StorageItemPermanent {
    fn total_bytes(&self) -> Result<u32, StorageItemError> {
        unreachable!()
    }

    fn write<'a>(
        &self,
        _buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError> {
        unreachable!()
    }

    fn read(variant: u8, _buffer: &SharedAlignedBuffer<Align16>) -> Result<Self, StorageItemError> {
        Err(StorageItemError::UnknownStorageItemVariant(variant))
    }
}
