#[cfg(test)]
mod tests;

use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::PageGroupKind;
use ab_blake3::single_block_hash;
use ab_core_primitives::hashes::Blake3Hash;
use blake3::hash;
use std::fmt;
use std::mem::MaybeUninit;

// TODO: use this
/// The minimum overhead that the storage item will have due to the way it is stored on disk
#[expect(dead_code, reason = "Not used yet")]
pub(crate) const fn min_segment_item_overhead() -> usize {
    // Align buffer used by storage item to 128 bytes
    let prefix_size = StorageItemContainer::<()>::prefix_size().next_multiple_of(size_of::<u128>());

    prefix_size + StorageItemContainer::<()>::suffix_size()
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StorageItemError {
    /// Buffer too small
    #[error("Buffer too small: expected {expected}, actual {actual}")]
    BufferTooSmall { expected: usize, actual: usize },
    /// Buffer too large
    #[error("Buffer too large, {extra_pages} extra pages were provided")]
    BufferTooLarge { extra_pages: usize },
    /// Need more bytes
    #[error("Need {0} more bytes")]
    NeedMoreBytes(usize),
    /// Unknown storage item variant
    #[error("Unknown storage item variant {0}")]
    UnknownStorageItemVariant(u8),
    /// Invalid data length
    #[error("Invalid data length {data_type}: expected {expected}, actual {actual}")]
    InvalidDataLength {
        data_type: &'static str,
        expected: usize,
        actual: usize,
    },
    /// Invalid data alignment
    #[error("Invalid data alignment {data_type}")]
    InvalidDataAlignment { data_type: &'static str },
    /// Checksum mismatch
    #[error("Checksum mismatch: expected {expected}, actual {actual}")]
    ChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Repeat checksum mismatch
    #[error("Repeat checksum mismatch: expected {expected}, actual {actual}")]
    RepeatChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Storage item checksum mismatch
    #[error("Storage item checksum mismatch: expected {expected}, actual {actual}")]
    StorageItemChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Invalid buffer contents
    #[error("Invalid buffer contents")]
    InvalidBufferContents,
    /// Storage item is too large
    #[error("Storage item is too large")]
    StorageItemTooLarge,
}

/// The result of [`StorageItem::write()`] call
pub(crate) struct StorageItemWriteResult<'a> {
    /// Storage item variant
    pub(crate) storage_item_variant: u8,
    /// Bytes where storage item was written
    pub(crate) storage_item_bytes: &'a [u8],
    /// Remaining bytes of the input buffer
    pub(crate) buffer: &'a mut [MaybeUninit<u8>],
}

pub(crate) trait StorageItem: fmt::Debug + Send + Sync + Sized + 'static {
    /// Total number of bytes
    fn total_bytes(&self) -> Result<u32, StorageItemError>;

    /// Write a storage item to the provided buffer.
    ///
    /// The buffer is required to be aligned to 128-bit.
    ///
    /// Returns the storage item variant and the number of bytes written.
    fn write<'a>(
        &self,
        buffer: &'a mut [MaybeUninit<u8>],
    ) -> Result<StorageItemWriteResult<'a>, StorageItemError>;

    /// The inverse of [`Self::write()`]
    fn read(variant: u8, buffer: &[u8]) -> Result<Self, StorageItemError>;
}

/// Storage item that maps to a unique page group kind
pub(crate) trait UniqueStorageItem: StorageItem {
    /// Unique page group for this storage item
    fn page_group_kind() -> PageGroupKind;
}

#[derive(Debug)]
pub(super) struct StorageItemContainer<SI> {
    pub(super) sequence_number: u64,
    pub(super) storage_item: SI,
}

impl<SI> StorageItemContainer<SI> {
    const fn prefix_size() -> usize {
        // Sequence number + enum variant + storage item size + checksum
        size_of::<u64>() + size_of::<u8>() + size_of::<u32>() + size_of::<Blake3Hash>()
    }

    const fn suffix_size() -> usize {
        // Storage item checksum + repeat of prefix checksum
        size_of::<Blake3Hash>() * 2
    }
}

impl<SI> StorageItemContainer<SI>
where
    SI: StorageItem,
{
    /// Returns the number of pages necessary to write this storage item
    pub(super) fn num_pages(&self) -> Result<u32, StorageItemError> {
        let storage_item_size = self.storage_item.total_bytes()?;

        // Align buffer used by storage item to 128 bytes
        let prefix_size = Self::prefix_size().next_multiple_of(size_of::<u128>());

        let num_bytes = storage_item_size
            .checked_add((prefix_size + Self::suffix_size()) as u32)
            .ok_or(StorageItemError::StorageItemTooLarge)?;

        Ok(num_bytes.div_ceil(AlignedPage::SIZE as u32))
    }

    /// Write a storage item to the provided buffer of aligned pages.
    ///
    /// Successful write means all provided pages are fully initialized.
    pub(super) fn write_to_pages(
        &self,
        buffer: &mut [MaybeUninit<AlignedPage>],
    ) -> Result<(), StorageItemError> {
        let mut buffer = buffer.as_bytes_mut();

        // Align buffer used by storage item to 128 bytes
        let prefix_bytes = buffer
            .split_off_mut(..Self::prefix_size().next_multiple_of(size_of::<u128>()))
            .expect("Always fits one page; qed");
        let (prefix_bytes, prefix_padding_bytes) = prefix_bytes.split_at_mut(Self::prefix_size());
        prefix_padding_bytes.write_filled(0);

        let StorageItemWriteResult {
            storage_item_variant,
            storage_item_bytes,
            mut buffer,
        } = self.storage_item.write(buffer)?;

        let buffer_len = buffer.len();
        let suffix_bytes = buffer
            .split_off_mut(..Self::suffix_size())
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::suffix_size() - buffer_len))?;

        let (before_checksum, checksum_bytes) =
            prefix_bytes.split_at_mut(size_of::<u64>() + size_of::<u8>() + size_of::<u32>());
        let (sequence_number_bytes, remainder) = before_checksum.split_at_mut(size_of::<u64>());
        let (storage_item_variant_bytes, storage_item_size_bytes) =
            remainder.split_at_mut(size_of::<u8>());

        // Write prefix
        sequence_number_bytes.write_copy_of_slice(&self.sequence_number.to_le_bytes());
        storage_item_variant_bytes[0].write(storage_item_variant);
        let storage_item_size = u32::try_from(storage_item_bytes.len())
            .map_err(|_error| StorageItemError::StorageItemTooLarge)?;
        storage_item_size_bytes.write_copy_of_slice(&storage_item_size.to_le_bytes());
        // SAFETY: Wrote to all components of `before_checksum` above
        let checksum = single_block_hash(unsafe { before_checksum.assume_init_ref() })
            .expect("Less than one block worth of data; qed");
        checksum_bytes.write_copy_of_slice(&checksum);

        let (storage_item_checksum_bytes, prefix_checksum_repeat_bytes) =
            suffix_bytes.split_at_mut(size_of::<Blake3Hash>());

        // Write suffix
        let storage_item_checksum = *hash(storage_item_bytes).as_bytes();
        storage_item_checksum_bytes.write_copy_of_slice(&storage_item_checksum);
        prefix_checksum_repeat_bytes.write_copy_of_slice(&checksum);

        if buffer.len() < AlignedPage::SIZE {
            buffer.write_filled(0);
        } else {
            return Err(StorageItemError::BufferTooLarge {
                extra_pages: buffer.len(),
            });
        }

        Ok(())
    }

    /// The inverse of [`Self::write_to_pages()`].
    ///
    /// Returns the storage item container and the number of pages it occupies.
    pub(super) fn read_from_pages(pages: &[AlignedPage]) -> Result<(Self, u32), StorageItemError> {
        let pages_bytes = AlignedPage::slice_to_repr(pages).as_flattened();
        let mut buffer = pages_bytes;

        // Align buffer used by storage item to 128 bytes
        let prefix_bytes = buffer
            .split_off(..Self::prefix_size().next_multiple_of(size_of::<u128>()))
            .expect("Always fits one page; qed");
        let prefix_bytes = &prefix_bytes[..Self::prefix_size()];

        let (sequence_number_bytes, remainder) = prefix_bytes.split_at(size_of::<u64>());
        let (storage_item_variant_bytes, remainder) = remainder.split_at(size_of::<u8>());
        let (storage_item_size_bytes, checksum_bytes) = remainder.split_at(size_of::<u32>());
        let checksum = Blake3Hash::new(
            single_block_hash(&prefix_bytes[..prefix_bytes.len() - checksum_bytes.len()])
                .expect("Less than one block worth of data; qed"),
        );

        if checksum.as_slice() != checksum_bytes {
            return Err(StorageItemError::ChecksumMismatch {
                expected: checksum,
                actual: Blake3Hash::new(checksum_bytes.try_into().expect("Correct length; qed")),
            });
        }

        let sequence_number = u64::from_le_bytes(
            sequence_number_bytes
                .try_into()
                .expect("Correct length; qed"),
        );
        let storage_item_variant = storage_item_variant_bytes[0];
        let storage_item_size = u32::from_le_bytes(
            storage_item_size_bytes
                .try_into()
                .expect("Correct length; qed"),
        );

        let buffer_len = buffer.len();
        let storage_item_size = storage_item_size as usize;
        let storage_item_bytes = buffer
            .split_off(..storage_item_size)
            .ok_or_else(|| StorageItemError::NeedMoreBytes(storage_item_size - buffer_len))?;

        let buffer_len = buffer.len();
        let suffix_bytes = buffer
            .split_off(..Self::suffix_size())
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::suffix_size() - buffer_len))?;
        let (storage_item_checksum_bytes, prefix_checksum_repeat_bytes) =
            suffix_bytes.split_at(size_of::<Blake3Hash>());

        if checksum.as_slice() != prefix_checksum_repeat_bytes {
            return Err(StorageItemError::RepeatChecksumMismatch {
                expected: checksum,
                actual: Blake3Hash::new(
                    prefix_checksum_repeat_bytes
                        .try_into()
                        .expect("Correct length; qed"),
                ),
            });
        }

        let storage_item_checksum = Blake3Hash::from(hash(storage_item_bytes));
        if storage_item_checksum.as_slice() != storage_item_checksum_bytes {
            return Err(StorageItemError::StorageItemChecksumMismatch {
                expected: storage_item_checksum,
                actual: Blake3Hash::new(
                    storage_item_checksum_bytes
                        .try_into()
                        .expect("Correct length; qed"),
                ),
            });
        }

        let storage_item = SI::read(storage_item_variant, storage_item_bytes)?;
        let num_pages =
            u32::try_from((pages_bytes.len() - buffer.len()).div_ceil(AlignedPage::SIZE)).expect(
                "Read bytes are the prefix, storage item of `u32` size and suffix, which take far \
                fewer than `u32::MAX` pages; qed",
            );

        Ok((
            Self {
                sequence_number,
                storage_item,
            },
            num_pages,
        ))
    }
}
