use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_core_primitives::segments::SuperSegmentHeader;
use ab_io_type::trivial_type::TrivialType;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub(crate) struct StorageItemBlockSuperSegmentHeaders {
    pub(crate) super_segment_headers: Vec<SuperSegmentHeader>,
}

impl StorageItemBlockSuperSegmentHeaders {
    pub(super) fn total_bytes(&self) -> usize {
        Self::prefix_size() + size_of_val(self.super_segment_headers.as_slice())
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
        let total_bytes = self.total_bytes();

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

            num_super_segment_headers
                .write_copy_of_slice(&(self.super_segment_headers.len() as u32).to_le_bytes());
        }

        // Write content bytes
        {
            let super_segment_headers_bytes = buffer
                .split_off_mut(..size_of_val(self.super_segment_headers.as_slice()))
                .expect("Total length checked above; qed");

            // TODO: It'd be nice to not have `size_of()` call, but there is no
            //  `MaybeUninit::transpose_mut()` or similar for type inference to work
            for (bytes, super_segment_header) in super_segment_headers_bytes
                .as_chunks_mut::<{ size_of::<SuperSegmentHeader>() }>()
                .0
                .iter_mut()
                .zip(&self.super_segment_headers)
            {
                bytes.write_copy_of_slice(super_segment_header.as_bytes());
            }
        }

        Ok(total_bytes)
    }

    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes = buffer
            .split_off(..Self::prefix_size())
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::prefix_size() - buffer_len))?;

        let (num_super_segment_headers, mut remainder) = prefix_bytes.split_at(size_of::<u32>());

        // Read the number of super segment headers
        let num_super_segment_headers = u32::from_le_bytes(
            num_super_segment_headers
                .try_into()
                .expect("Correct length; qed"),
        ) as usize;

        let mut super_segment_headers = Vec::with_capacity(num_super_segment_headers);

        for _ in 0..num_super_segment_headers {
            let buffer_len = buffer.len();
            let super_segment_header_bytes = remainder
                .split_off(..size_of::<SuperSegmentHeader>())
                .ok_or(StorageItemError::NeedMoreBytes(
                    size_of::<SuperSegmentHeader>() - buffer_len,
                ))?;
            // TODO: Would be nice to have slice API in `TrivialType`
            // SAFETY: This is a local database, so anything that is read that passes checksum
            // verification is valid
            let super_segment_header = *unsafe {
                SuperSegmentHeader::from_bytes(super_segment_header_bytes).ok_or(
                    StorageItemError::InvalidDataAlignment {
                        data_type: "SuperSegmentHeader",
                    },
                )?
            };

            super_segment_headers.push(super_segment_header);
        }

        Ok(Self {
            super_segment_headers,
        })
    }
}
