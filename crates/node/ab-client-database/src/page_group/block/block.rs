use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::BlockMerkleMountainRange;
use ab_merkle_tree::mmr::MerkleMountainRangeBytes;
use rclite::Arc;

#[derive(Debug)]
pub(crate) struct StorageItemBlockBlock {
    pub(crate) header: SharedAlignedBuffer,
    pub(crate) body: SharedAlignedBuffer,
    pub(crate) mmr_with_block: Arc<BlockMerkleMountainRange>,
    // TODO: State, segment headers
}

impl StorageItemBlockBlock {
    pub(super) fn total_bytes(&self) -> usize {
        Self::total_bytes_inner(
            self.header.len(),
            self.body.len(),
            self.mmr_with_block.as_bytes().len() as u32,
        )
    }

    const fn total_bytes_inner(header_len: u32, body_len: u32, mmr_len: u32) -> usize {
        Self::block_prefix_size() + Self::block_content_size(header_len, body_len, mmr_len)
    }

    const fn block_prefix_size() -> usize {
        // 3 lengths of header/block/mmr
        size_of::<u32>()
    }

    const fn block_content_size(header_len: u32, body_len: u32, mmr_len: u32) -> usize {
        header_len as usize + body_len as usize + mmr_len as usize
    }

    pub(super) fn write(&self, mut buffer: &mut [u8]) -> Result<usize, StorageItemError> {
        let total_bytes = self.total_bytes();

        if buffer.len() < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer.len(),
            });
        }

        let mmr_with_block = self.mmr_with_block.as_bytes();

        // TODO: Take offsets into consideration, header and body must start at multiple of u128/16
        //  bytes to support memory-mapped reading without extra copies
        let contents = [
            self.header.as_slice(),
            self.body.as_slice(),
            mmr_with_block.as_slice(),
        ];
        // Write all lengths
        for bytes in contents {
            let length = bytes.len() as u32;
            buffer
                .split_off_mut(..size_of_val(&length))
                .expect("Enough memory, checked above; qed")
                .copy_from_slice(&u32::to_le_bytes(length));
        }
        // Write content bytes
        for bytes in contents {
            buffer
                .split_off_mut(..size_of_val(bytes))
                .expect("Enough memory, checked above; qed")
                .copy_from_slice(bytes);
        }

        Ok(total_bytes)
    }

    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes = buffer.split_off(..Self::block_prefix_size()).ok_or(
            StorageItemError::NeedMoreBytes(buffer_len - Self::block_prefix_size()),
        )?;

        let (header_len, remainder) = prefix_bytes.split_at(size_of::<u32>());
        let (body_len, mmr_len) = remainder.split_at(size_of::<u32>());

        // Read lengths
        let header_len = u32::from_le_bytes(header_len.try_into().expect("Correct length; qed"));
        let body_len = u32::from_le_bytes(body_len.try_into().expect("Correct length; qed"));
        let mmr_len = u32::from_le_bytes(mmr_len.try_into().expect("Correct length; qed"));

        let buffer_len = buffer.len();
        let content_size = Self::block_content_size(header_len, body_len, mmr_len);
        let mut content_bytes = buffer
            .split_off(..content_size)
            .ok_or(StorageItemError::NeedMoreBytes(buffer_len - content_size))?;

        // Read contents bytes
        let header = SharedAlignedBuffer::from_bytes(
            content_bytes
                .split_off(..header_len as usize)
                .expect("Just checked to have enough bytes; qed"),
        );
        let body = SharedAlignedBuffer::from_bytes(
            content_bytes
                .split_off(..body_len as usize)
                .expect("Just checked to have enough bytes; qed"),
        );
        let mmr_raw_bytes = content_bytes
            .split_off(..mmr_len as usize)
            .expect("Just checked to have enough bytes; qed");
        let mmr = {
            let mut mmr_bytes = MerkleMountainRangeBytes::default();

            if mmr_bytes.len() != mmr_raw_bytes.len() {
                return Err(StorageItemError::InvalidMmrLength(
                    mmr_raw_bytes.len() as u32
                ));
            }

            mmr_bytes.copy_from_slice(mmr_raw_bytes);

            // SAFETY: Created using `BlockMerkleMountainRange::as_bytes()` and checked data
            // integrity
            *unsafe { BlockMerkleMountainRange::from_bytes(&mmr_bytes) }
        };

        Ok(Self {
            header,
            body,
            mmr_with_block: Arc::new(mmr),
        })
    }
}
