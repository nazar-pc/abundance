use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_core_primitives::address::Address;
use ab_core_primitives::segments::SegmentHeader;
use ab_io_type::trivial_type::TrivialType;
use std::mem::MaybeUninit;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
struct SystemContractStatePrefix {
    owner: Address,
    contract: Address,
    content_len: u32,
    padding: [u8; 4],
}

const _: () = {
    assert!(align_of::<SystemContractStatePrefix>() == align_of::<u64>());
};

#[derive(Debug)]
pub(crate) struct StorageItemBlockSegmentHeaders {
    pub(crate) segment_headers: Vec<SegmentHeader>,
}

impl StorageItemBlockSegmentHeaders {
    pub(super) fn total_bytes(&self) -> usize {
        Self::prefix_size() + size_of_val(self.segment_headers.as_slice())
    }

    const fn prefix_size() -> usize {
        const PREFIX_SIZE: usize = size_of::<u32>();
        const {
            // Ensure that writing segment headers after `u32` length prefix will result in
            // correctly aligned data
            assert!(align_of::<SegmentHeader>() == PREFIX_SIZE);
        }
        PREFIX_SIZE
    }

    pub(super) fn write(
        &self,
        mut buffer: &mut [MaybeUninit<u8>],
    ) -> Result<usize, StorageItemError> {
        // The layout here is as follows:
        // * number of segment headers: u32 as aligned little-endian bytes
        // * segment headers bytes concatenated

        let buffer_len = buffer.len();
        let total_bytes = self.total_bytes();

        if buffer_len < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer_len,
            });
        }

        // Write the number of segment headers
        {
            let num_segment_headers = buffer
                .split_off_mut(..Self::prefix_size())
                .expect("Total length checked above; qed");

            num_segment_headers
                .write_copy_of_slice(&(self.segment_headers.len() as u32).to_le_bytes());
        }

        // Write content bytes
        {
            let segment_headers_bytes = buffer
                .split_off_mut(..size_of_val(self.segment_headers.as_slice()))
                .expect("Total length checked above; qed");

            for (bytes, segment_header) in segment_headers_bytes
                // TODO: Constant will be inferred once `.as_bytes()` returns an array instead of
                //  slice
                .as_chunks_mut::<{ size_of::<SegmentHeader>() }>()
                .0
                .iter_mut()
                .zip(&self.segment_headers)
            {
                bytes.write_copy_of_slice(segment_header.as_bytes());
            }
        }

        Ok(total_bytes)
    }

    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes = buffer
            .split_off(..Self::prefix_size())
            .ok_or_else(|| StorageItemError::NeedMoreBytes(Self::prefix_size() - buffer_len))?;

        let (num_segment_headers, mut remainder) = prefix_bytes.split_at(size_of::<u32>());

        // Read the number of segment headers
        let num_segment_headers =
            u32::from_le_bytes(num_segment_headers.try_into().expect("Correct length; qed"))
                as usize;

        let mut segment_headers = Vec::with_capacity(num_segment_headers);

        for _ in 0..num_segment_headers {
            let buffer_len = buffer.len();
            let segment_header_bytes = remainder.split_off(..size_of::<SegmentHeader>()).ok_or(
                StorageItemError::NeedMoreBytes(size_of::<SegmentHeader>() - buffer_len),
            )?;
            // TODO: Would be nice to have slice API in `TrivialType`
            // SAFETY: This is a local database, so anything that is read that passes checksum
            // verification is valid
            let segment_header = *unsafe {
                SegmentHeader::from_bytes(segment_header_bytes).ok_or(
                    StorageItemError::InvalidDataAlignment {
                        data_type: "SegmentHeader",
                    },
                )?
            };

            segment_headers.push(segment_header);
        }

        Ok(Self { segment_headers })
    }
}
