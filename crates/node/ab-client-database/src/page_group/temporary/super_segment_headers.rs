#[cfg(test)]
mod tests;

use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_core_primitives::segments::SuperSegmentHeader;
use ab_io_type::trivial_type::TrivialType;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub(crate) struct StorageItemTemporarySuperSegmentHeaders {
    pub(crate) super_segment_headers: Vec<SuperSegmentHeader>,
}

impl StorageItemTemporarySuperSegmentHeaders {
    pub(super) fn total_bytes(&self) -> Result<u32, StorageItemError> {
        u32::try_from(size_of_val(self.super_segment_headers.as_slice()))
            .ok()
            .and_then(|super_segment_headers_len| {
                super_segment_headers_len.checked_add(Self::prefix_size() as u32)
            })
            .ok_or(StorageItemError::StorageItemTooLarge)
    }

    const fn prefix_size() -> usize {
        const PREFIX_SIZE: usize = size_of::<u32>();
        const {
            // Ensure that writing super segment headers after `u32` length prefix will result in
            // correctly aligned data
            assert!(align_of::<SuperSegmentHeader>() == PREFIX_SIZE);
        }
        PREFIX_SIZE
    }

    pub(super) fn write(
        &self,
        mut buffer: &mut [MaybeUninit<u8>],
    ) -> Result<usize, StorageItemError> {
        // The layout here is as follows:
        // * number of super segment headers: u32 as aligned little-endian bytes
        // * super segment headers bytes concatenated

        let buffer_len = buffer.len();
        let total_bytes = self.total_bytes()? as usize;

        if buffer_len < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer_len,
            });
        }

        // Write the number of super segment headers
        {
            let num_super_segment_headers = buffer
                .split_off_mut(..Self::prefix_size())
                .expect("Total length checked above; qed");

            num_super_segment_headers.write_copy_of_slice(
                &u32::try_from(self.super_segment_headers.len())
                    .expect(
                        "Each super segment header takes at least one byte out of \
                        `Self::total_bytes()`, which is `u32`; qed",
                    )
                    .to_le_bytes(),
            );
        }

        // Write content bytes
        {
            let super_segment_headers_bytes = buffer
                .split_off_mut(..size_of_val(self.super_segment_headers.as_slice()))
                .expect("Total length checked above; qed");

            super_segment_headers_bytes.write_copy_of_slice(SuperSegmentHeader::slice_as_bytes(
                &self.super_segment_headers,
            ));
        }

        Ok(total_bytes)
    }

    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes = buffer
            .split_off(..Self::prefix_size())
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::prefix_size() - buffer_len))?;

        // Read the number of super segment headers
        let num_super_segment_headers =
            u32::from_le_bytes(prefix_bytes.try_into().expect("Correct length; qed")) as usize;

        let mut super_segment_headers = Vec::with_capacity(num_super_segment_headers);

        for _ in 0..num_super_segment_headers {
            let buffer_len = buffer.len();
            let super_segment_header_bytes = buffer
                .split_off(..size_of::<SuperSegmentHeader>())
                .ok_or_else(|| {
                    StorageItemError::NeedMoreBytes(size_of::<SuperSegmentHeader>() - buffer_len)
                })?;
            // TODO: Would be nice to have slice API in `TrivialType`
            let super_segment_header = *SuperSegmentHeader::from_bytes_exact(
                super_segment_header_bytes,
            )
            .ok_or(StorageItemError::InvalidDataAlignment {
                data_type: "SuperSegmentHeader",
            })?;

            super_segment_headers.push(super_segment_header);
        }

        Ok(Self {
            super_segment_headers,
        })
    }
}
