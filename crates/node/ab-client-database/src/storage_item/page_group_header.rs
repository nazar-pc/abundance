use crate::DatabaseId;
use crate::storage_item::StorageItemError;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
pub(crate) enum PageGroupKind {
    Permanent = 0,
    Ephemeral = 1,
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub(crate) struct StorageItemPageGroupHeader {
    /// Database ID.
    ///
    /// Must be the same for all pages in a database.
    pub(crate) database_id: DatabaseId,
    /// Database version
    pub(crate) version: u8,
    /// The kind of page group
    pub(crate) kind: PageGroupKind,
    // Padding for data alignment
    pub(crate) padding: [u8; 2],
    /// The number of pages in a page group
    pub(crate) page_group_size: u32,
}

impl StorageItemPageGroupHeader {
    pub(crate) const fn total_bytes(&self) -> usize {
        size_of::<Self>()
    }

    /// Write a storage item to the provided buffer.
    ///
    /// Returns the number of bytes written.
    pub(super) fn write(&self, buffer: &mut [u8]) -> Result<usize, StorageItemError> {
        let total_bytes = size_of::<Self>();

        if buffer.len() < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer.len(),
            });
        }

        buffer[..total_bytes].copy_from_slice(self.as_bytes());

        Ok(total_bytes)
    }

    /// The inverse of [`Self::write_to_pages()`]
    pub(super) fn read(buffer: &[u8]) -> Result<Self, StorageItemError> {
        // SAFETY: All bit pattens are valid
        let maybe_item = unsafe { Self::from_bytes(buffer) };
        let item = *maybe_item.ok_or(StorageItemError::BufferTooSmall {
            expected: size_of::<Self>(),
            actual: buffer.len(),
        })?;

        Ok(item)
    }
}
