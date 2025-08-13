use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::{BlockMerkleMountainRange, ContractSlotState};
use ab_core_primitives::address::Address;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::mmr::MerkleMountainRangeBytes;
use rclite::Arc;
use std::mem::MaybeUninit;
use std::sync::Arc as StdArc;

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
pub(crate) struct StorageItemBlockBlock {
    pub(crate) header: SharedAlignedBuffer,
    pub(crate) body: SharedAlignedBuffer,
    pub(crate) mmr_with_block: Arc<BlockMerkleMountainRange>,
    pub(crate) system_contract_states: StdArc<[ContractSlotState]>,
    // TODO: State, segment headers
}

impl StorageItemBlockBlock {
    pub(super) fn total_bytes(&self) -> usize {
        Self::total_bytes_inner(
            self.header.len(),
            self.body.len(),
            self.mmr_with_block.as_bytes().len() as u32,
            self.system_contract_states_len(),
        )
    }

    fn system_contract_states_len(&self) -> u32 {
        let mut len = 0u32;
        for system_contract_state in self.system_contract_states.as_ref() {
            len = len.next_multiple_of(size_of::<u64>() as u32);
            len += SystemContractStatePrefix::SIZE;
            len = len.next_multiple_of(size_of::<u128>() as u32);
            len += system_contract_state.contents.len();
        }
        len
    }

    const fn total_bytes_inner(
        header_len: u32,
        body_len: u32,
        mmr_len: u32,
        system_contract_states_len: u32,
    ) -> usize {
        Self::block_prefix_size()
            + Self::block_content_size(header_len, body_len, mmr_len, system_contract_states_len)
    }

    const fn block_prefix_size() -> usize {
        // 4 lengths of header/block/mmr/num system contracts states
        const BLOCK_PREFIX_SIZE: usize = size_of::<u32>() * 4;
        // Ensure always aligned to `u128`
        const _: () = {
            assert!(BLOCK_PREFIX_SIZE == size_of::<u128>());
        };
        BLOCK_PREFIX_SIZE
    }

    const fn block_content_size(
        header_len: u32,
        body_len: u32,
        mmr_len: u32,
        system_contract_states_len: u32,
    ) -> usize {
        // Account for alignment
        let len = (header_len as usize).next_multiple_of(size_of::<u128>())
            + body_len as usize
            + mmr_len as usize;
        len.next_multiple_of(size_of::<u64>()) + system_contract_states_len as usize
    }

    pub(super) fn write(
        &self,
        mut buffer: &mut [MaybeUninit<u8>],
    ) -> Result<usize, StorageItemError> {
        // The layout here is as follows:
        // * header length: u32 as aligned little-endian bytes
        // * body length: u32 as aligned little-endian bytes
        // * MMR with block length: u32 as aligned little-endian bytes
        // * number of system contract states: u32 as aligned little-endian bytes
        // * block header: naturally aligned to 16-bytes boundary
        // * padding to 16-bytes boundary (if needed)
        // * block body
        // * MMR with block bytes
        // * for each system contract state:
        //   * padding to the 8-bytes boundary (if needed)
        //   * prefix: SystemContractStatePrefix
        //   * padding to the 16-bytes boundary (if needed)
        //   * contents: slot contents bytes

        let buffer_len = buffer.len();
        let total_bytes = self.total_bytes();

        if buffer_len < total_bytes {
            return Err(StorageItemError::BufferTooSmall {
                expected: total_bytes,
                actual: buffer_len,
            });
        }

        let header = self.header.as_slice();
        let body = self.body.as_slice();
        let mmr_with_block = self.mmr_with_block.as_bytes().as_slice();
        let system_contract_states = self.system_contract_states.as_ref();
        let mut written_len = 0usize;

        // Write all lengths
        {
            let prefix_bytes = buffer
                .split_off_mut(..Self::block_prefix_size())
                .expect("Total length checked above; qed");
            let (header_len, remainder) = prefix_bytes.split_at_mut(size_of::<u32>());
            let (body_len, remainder) = remainder.split_at_mut(size_of::<u32>());
            let (mmr_len, num_system_contract_states) = remainder.split_at_mut(size_of::<u32>());

            header_len.write_copy_of_slice(&(header.len() as u32).to_le_bytes());
            body_len.write_copy_of_slice(&(body.len() as u32).to_le_bytes());
            mmr_len.write_copy_of_slice(&(mmr_with_block.len() as u32).to_le_bytes());
            num_system_contract_states
                .write_copy_of_slice(&(system_contract_states.len() as u32).to_le_bytes());

            written_len += prefix_bytes.len();
        }

        // Write content bytes
        {
            let header_bytes = buffer
                .split_off_mut(..header.len().next_multiple_of(size_of::<u128>()))
                .expect("Total length checked above; qed");

            // Sub-slice due to possible trailing alignment bytes
            header_bytes[..header.len()].write_copy_of_slice(header);
            written_len += header_bytes.len();
        }
        {
            let body_bytes = buffer
                .split_off_mut(..body.len())
                .expect("Total length checked above; qed");

            body_bytes.write_copy_of_slice(body);
            written_len += body_bytes.len();
        }
        {
            let mmr_raw_bytes = buffer
                .split_off_mut(..mmr_with_block.len())
                .expect("Total length checked above; qed");

            // Sub-slice due to possible trailing alignment bytes
            mmr_raw_bytes.write_copy_of_slice(mmr_with_block);
            written_len += mmr_raw_bytes.len();
        }

        for system_contract_state in system_contract_states {
            // Alignment padding (if needed)
            if !written_len.is_multiple_of(size_of::<u64>()) {
                let new_written_len = written_len.next_multiple_of(size_of::<u64>());
                buffer
                    .split_off_mut(..(new_written_len - written_len))
                    .expect("Total length checked above; qed");
                written_len = new_written_len;
            }

            {
                let prefix_bytes = buffer
                    .split_off_mut(..size_of::<SystemContractStatePrefix>())
                    .expect("Total length checked above; qed");
                prefix_bytes.write_copy_of_slice(
                    SystemContractStatePrefix {
                        owner: system_contract_state.owner,
                        contract: system_contract_state.contract,
                        content_len: system_contract_state.contents.len(),
                        padding: [0; _],
                    }
                    .as_bytes(),
                );
                written_len += prefix_bytes.len();
            }

            // Alignment padding (if needed)
            if !written_len.is_multiple_of(size_of::<u128>()) {
                let new_written_len = written_len.next_multiple_of(size_of::<u128>());
                buffer
                    .split_off_mut(..(new_written_len - written_len))
                    .expect("Total length checked above; qed");
                written_len = new_written_len;
            }

            {
                let contents_bytes = buffer
                    .split_off_mut(..system_contract_state.contents.len() as usize)
                    .expect("Total length checked above; qed");
                contents_bytes.write_copy_of_slice(system_contract_state.contents.as_slice());
                written_len += contents_bytes.len();
            }
        }

        Ok(total_bytes)
    }

    pub(super) fn read(mut buffer: &[u8]) -> Result<Self, StorageItemError> {
        let buffer_len = buffer.len();
        let prefix_bytes = buffer.split_off(..Self::block_prefix_size()).ok_or(
            StorageItemError::NeedMoreBytes(buffer_len - Self::block_prefix_size()),
        )?;
        let mut read_len = prefix_bytes.len();

        let (header_len, remainder) = prefix_bytes.split_at(size_of::<u32>());
        let (body_len, remainder) = remainder.split_at(size_of::<u32>());
        let (mmr_len, num_system_contract_states) = remainder.split_at(size_of::<u32>());

        // Read lengths
        let header_len =
            u32::from_le_bytes(header_len.try_into().expect("Correct length; qed")) as usize;
        let body_len =
            u32::from_le_bytes(body_len.try_into().expect("Correct length; qed")) as usize;
        let mmr_len = u32::from_le_bytes(mmr_len.try_into().expect("Correct length; qed")) as usize;
        let num_system_contract_states_len = u32::from_le_bytes(
            num_system_contract_states
                .try_into()
                .expect("Correct length; qed"),
        );

        let header = {
            let buffer_len = buffer.len();
            let header_bytes = buffer
                .split_off(..header_len.next_multiple_of(size_of::<u128>()))
                .ok_or(StorageItemError::NeedMoreBytes(
                    buffer_len - header_len.next_multiple_of(size_of::<u128>()),
                ))?;
            let header = SharedAlignedBuffer::from_bytes(&header_bytes[..header_len]);
            read_len += header_bytes.len();
            header
        };

        let body = {
            let buffer_len = buffer.len();
            let body_bytes = buffer
                .split_off(..body_len)
                .ok_or(StorageItemError::NeedMoreBytes(buffer_len - body_len))?;
            let body = SharedAlignedBuffer::from_bytes(body_bytes);
            read_len += body_bytes.len();
            body
        };

        let mmr = {
            let buffer_len = buffer.len();
            let mmr_raw_bytes = buffer
                .split_off(..mmr_len)
                .ok_or(StorageItemError::NeedMoreBytes(buffer_len - mmr_len))?;

            let mut mmr_bytes = MerkleMountainRangeBytes::default();

            if mmr_bytes.len() != mmr_raw_bytes.len() {
                return Err(StorageItemError::InvalidDataLength {
                    data_type: "MerkleMountainRangeBytes",
                    expected: mmr_bytes.len(),
                    actual: mmr_raw_bytes.len(),
                });
            }

            mmr_bytes.copy_from_slice(mmr_raw_bytes);

            // SAFETY: Created using `BlockMerkleMountainRange::as_bytes()` and checked data
            // integrity
            let mmr = unsafe { BlockMerkleMountainRange::from_bytes(&mmr_bytes) };
            read_len += mmr_raw_bytes.len();
            *mmr
        };

        let mut system_contract_states = StdArc::<[ContractSlotState]>::new_uninit_slice(
            num_system_contract_states_len as usize,
        );

        // SAFETY: A single pointer and a single use
        for system_contract_state in
            unsafe { StdArc::get_mut_unchecked(&mut system_contract_states) }
        {
            // Alignment padding (if needed)
            if !read_len.is_multiple_of(size_of::<u64>()) {
                let new_read_len = read_len.next_multiple_of(size_of::<u64>());
                let buffer_len = buffer.len();
                buffer.split_off(..(new_read_len - read_len)).ok_or(
                    StorageItemError::NeedMoreBytes(buffer_len - (new_read_len - read_len)),
                )?;
                read_len = new_read_len;
            }

            let prefix = {
                let buffer_len = buffer.len();
                let prefix_bytes = buffer
                    .split_off(..size_of::<SystemContractStatePrefix>())
                    .ok_or(StorageItemError::NeedMoreBytes(
                        buffer_len - size_of::<SystemContractStatePrefix>(),
                    ))?;
                let prefix = unsafe {
                    SystemContractStatePrefix::from_bytes(prefix_bytes).ok_or(
                        StorageItemError::InvalidDataAlignment {
                            data_type: "SystemContractStatePrefix",
                        },
                    )?
                };
                read_len += prefix_bytes.len();
                prefix
            };

            // Alignment padding (if needed)
            if !read_len.is_multiple_of(size_of::<u128>()) {
                let new_read_len = read_len.next_multiple_of(size_of::<u128>());
                let buffer_len = buffer.len();
                buffer.split_off(..(new_read_len - read_len)).ok_or(
                    StorageItemError::NeedMoreBytes(buffer_len - (new_read_len - read_len)),
                )?;
                read_len = new_read_len;
            }

            let contents = {
                let buffer_len = buffer.len();
                let contents_bytes = buffer.split_off(..prefix.content_len as usize).ok_or(
                    StorageItemError::NeedMoreBytes(buffer_len - prefix.content_len as usize),
                )?;
                let contents = SharedAlignedBuffer::from_bytes(contents_bytes);
                read_len += contents_bytes.len();
                contents
            };

            system_contract_state.write(ContractSlotState {
                owner: prefix.owner,
                contract: prefix.contract,
                contents,
            });
        }

        // SAFETY: Just initialized all entries
        let system_contract_states = unsafe { system_contract_states.assume_init() };

        Ok(Self {
            header,
            body,
            mmr_with_block: Arc::new(mmr),
            system_contract_states,
        })
    }
}
