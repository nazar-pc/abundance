use crate::storage_backend_adapter::storage_item::{StorageItem, StorageItemError};
use crate::{DatabaseId, PageGroupKind};
use ab_io_type::trivial_type::TrivialType;
use std::mem;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub(crate) struct StorageItemPageGroupHeader {
    /// Database ID.
    ///
    /// Must be the same for all pages in a database.
    pub(crate) database_id: DatabaseId,
    /// Database version
    pub(crate) database_version: u8,
    /// The kind of page group
    pub(crate) page_group_kind: PageGroupKind,
    // Padding for data alignment
    pub(crate) padding: [u8; 2],
    /// The number of pages in a page group
    pub(crate) page_group_size: u32,
}

impl StorageItem for StorageItemPageGroupHeader {
    #[inline(always)]
    fn total_bytes(&self) -> usize {
        size_of::<Self>()
    }

    #[inline(always)]
    fn write(&self, buffer: &mut [u8]) -> Result<(u8, usize), StorageItemError> {
        let total_bytes = size_of::<Self>();

        if buffer.len() < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer.len(),
            });
        }

        buffer[..total_bytes].copy_from_slice(self.as_bytes());

        Ok((0, total_bytes))
    }

    #[inline(always)]
    fn read(variant: u8, buffer: &[u8]) -> Result<Self, StorageItemError> {
        if variant != 0 {
            return Err(StorageItemError::UnknownStorageItemVariant(variant));
        }

        if buffer.len() < size_of::<Self>() {
            return Err(StorageItemError::BufferTooSmall {
                expected: size_of::<Self>(),
                actual: buffer.len(),
            });
        }
        let kind_byte = buffer[mem::offset_of!(Self, page_group_kind)];
        PageGroupKind::from_repr(kind_byte).ok_or(StorageItemError::InvalidBufferContents)?;

        // SAFETY: `PageGroupKind` checked above, all other bit pattens are valid
        let maybe_item = unsafe { Self::from_bytes(buffer) };
        let item = *maybe_item.ok_or(StorageItemError::BufferTooSmall {
            expected: size_of::<Self>(),
            actual: buffer.len(),
        })?;

        Ok(item)
    }
}
