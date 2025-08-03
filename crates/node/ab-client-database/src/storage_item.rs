pub(crate) mod page_group_header;

use crate::storage_backend::AlignedPage;
use ab_blake3::single_block_hash;
use ab_core_primitives::hashes::Blake3Hash;
use blake3::hash;
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub(crate) enum StorageItemError {
    /// Buffer too small
    #[error("Buffer too small (expected: {expected}, actual: {actual})")]
    BufferTooSmall { expected: usize, actual: usize },
    /// Need more bytes
    #[error("Need {0} more bytes")]
    NeedMoreBytes(usize),
    /// Unknown storage item variant
    #[error("Unknown storage item variant {0}")]
    UnknownStorageItemVariant(u8),
    /// Invalid MMR length
    #[error("Invalid MMR length {0}")]
    InvalidMmrLength(u32),
    /// Checksum mismatch
    #[error("Checksum mismatch (expected: {expected}, actual: {actual})")]
    ChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Repeat checksum mismatch
    #[error("Repeat checksum mismatch (expected: {expected}, actual: {actual})")]
    RepeatChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Storage item checksum mismatch
    #[error("Storage item checksum mismatch (expected: {expected}, actual: {actual})")]
    StorageItemChecksumMismatch {
        expected: Blake3Hash,
        actual: Blake3Hash,
    },
    /// Invalid buffer contents
    #[error("Invalid buffer contents")]
    InvalidBufferContents,
}

pub(crate) trait StorageItemKind: fmt::Debug + Send + Sync + Sized + 'static {
    /// Total number of bytes
    fn total_bytes(&self) -> usize;

    /// Write a storage item to the provided buffer.
    ///
    /// The buffer is required to be aligned to 128-bit.
    ///
    /// Returns the storage item variant and the number of bytes written.
    fn write(&self, buffer: &mut [u8]) -> Result<(u8, usize), StorageItemError>;

    /// The inverse of [`Self::write_to_pages()`]
    fn read(variant: u8, buffer: &[u8]) -> Result<Self, StorageItemError>;
}

#[derive(Debug)]
pub(crate) struct StorageItem<Kind> {
    pub(crate) sequence_number: u64,
    pub(crate) storage_item_kind: Kind,
}

impl<Kind> StorageItem<Kind>
where
    Kind: StorageItemKind,
{
    /// Returns the number of pages necessary to write this storage item
    pub(crate) fn num_pages(&self) -> u32 {
        let storage_item_size = self.storage_item_kind.total_bytes();

        // Align buffer used by storage item to 128 bytes
        let prefix_size = Self::prefix_size().next_multiple_of(size_of::<u128>());

        (prefix_size + storage_item_size + Self::suffix_size()).div_ceil(AlignedPage::SIZE) as u32
    }

    const fn prefix_size() -> usize {
        // Sequence number + enum variant + storage item size + checksum
        size_of::<u64>() + size_of::<u8>() + size_of::<u32>() + size_of::<Blake3Hash>()
    }

    const fn suffix_size() -> usize {
        // Storage item checksum + repeat of prefix checksum
        size_of::<Blake3Hash>() * 2
    }

    /// Write a storage item to the provided buffer of aligned pages
    pub(crate) fn write_to_pages(
        &self,
        buffer: &mut [AlignedPage],
    ) -> Result<(), StorageItemError> {
        let mut buffer = AlignedPage::slice_mut_to_repr(buffer).as_flattened_mut();

        let prefix_bytes = buffer
            .split_off_mut(..Self::prefix_size())
            .expect("Always fits one page; qed");
        // Align buffer used by storage item to 128 bytes
        buffer = &mut buffer[Self::prefix_size().next_multiple_of(size_of::<u128>())..];

        let (storage_item_variant, storage_item_size) = self.storage_item_kind.write(buffer)?;
        let (storage_item_bytes, mut buffer) = buffer.split_at_mut(storage_item_size);

        let buffer_len = buffer.len();
        let suffix_bytes =
            buffer
                .split_off_mut(..Self::suffix_size())
                .ok_or(StorageItemError::NeedMoreBytes(
                    buffer_len - Self::suffix_size(),
                ))?;

        let (before_checksum, checksum_bytes) =
            prefix_bytes.split_at_mut(size_of::<u64>() + size_of::<u8>() + size_of::<u32>());
        let (sequence_number_bytes, remainder) = before_checksum.split_at_mut(size_of::<u64>());
        let (storage_item_variant_bytes, storage_item_size_bytes) =
            remainder.split_at_mut(size_of::<u8>());

        // Write prefix
        sequence_number_bytes.copy_from_slice(&self.sequence_number.to_le_bytes());
        storage_item_variant_bytes[0] = storage_item_variant;
        storage_item_size_bytes.copy_from_slice(&storage_item_size.to_le_bytes());
        let checksum =
            single_block_hash(before_checksum).expect("Less than one block worth of data; qed");
        checksum_bytes.copy_from_slice(&checksum);

        let (storage_item_checksum_bytes, prefix_checksum_repeat_bytes) =
            suffix_bytes.split_at_mut(size_of::<Blake3Hash>());

        // Write suffix
        let storage_item_checksum = *hash(storage_item_bytes).as_bytes();
        storage_item_checksum_bytes.copy_from_slice(&storage_item_checksum);
        prefix_checksum_repeat_bytes.copy_from_slice(&checksum);

        Ok(())
    }

    /// The inverse of [`Self::write_to_pages()`]
    pub(crate) fn read_from_pages(pages: &[AlignedPage]) -> Result<Self, StorageItemError> {
        let mut buffer = AlignedPage::slice_to_repr(pages).as_flattened();

        let prefix_bytes = buffer
            .split_off(..Self::prefix_size())
            .expect("Always fits one page; qed");
        // Align buffer used by storage item to 128 bytes
        buffer = &buffer[Self::prefix_size().next_multiple_of(size_of::<u128>())..];

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
        let storage_item_bytes = buffer.split_off(..storage_item_size as usize).ok_or(
            StorageItemError::NeedMoreBytes(buffer_len - storage_item_size as usize),
        )?;

        let buffer_len = buffer.len();
        let suffix_bytes =
            buffer
                .split_off(..Self::suffix_size())
                .ok_or(StorageItemError::NeedMoreBytes(
                    buffer_len - Self::suffix_size(),
                ))?;
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

        let storage_item_kind = Kind::read(storage_item_variant, storage_item_bytes)?;

        Ok(Self {
            sequence_number,
            storage_item_kind,
        })
    }
}
