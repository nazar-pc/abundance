#[cfg(test)]
mod tests;

use crate::storage_backend_adapter::storage_item::StorageItemError;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::{Align8, Align16};
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::trees::BlockMerkleMountainRange;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::mmr::MerkleMountainRangeBytes;
use std::mem::MaybeUninit;
use triomphe::Arc;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
struct SystemContractStatePrefix {
    owner: Address,
    contract: Address,
    content_len: u32,
    padding: [u8; 4],
}

const {
    assert!(align_of::<SystemContractStatePrefix>() == align_of::<u64>());
}

#[derive(Debug)]
pub(crate) struct StorageItemTemporaryBlock {
    pub(crate) header: SharedAlignedBuffer,
    pub(crate) body: SharedAlignedBuffer,
    pub(crate) mmr_with_block: Arc<BlockMerkleMountainRange>,
    pub(crate) system_contract_states: Arc<[ContractSlotState]>,
    // TODO: State, segment headers
}

impl StorageItemTemporaryBlock {
    pub(super) fn total_bytes(&self) -> Result<u32, StorageItemError> {
        let mmr_len = u32::try_from(self.mmr_with_block.as_bytes().len())
            .map_err(|_error| StorageItemError::StorageItemTooLarge)?;

        // Account for alignment
        let mut total_bytes = self
            .header
            .len()
            .checked_next_multiple_of(u128::SIZE)
            .and_then(|header_len| header_len.checked_add(Self::prefix_size() as u32))
            .and_then(|total_bytes| total_bytes.checked_add(self.body.len()))
            .and_then(|total_bytes| total_bytes.checked_add(mmr_len))
            .and_then(|total_bytes| total_bytes.checked_next_multiple_of(u64::SIZE))
            .ok_or(StorageItemError::StorageItemTooLarge)?;

        // Padding of system contract states depends on the offset they are written at
        for system_contract_state in self.system_contract_states.as_ref() {
            total_bytes = total_bytes
                .checked_next_multiple_of(u64::SIZE)
                .and_then(|total_bytes| total_bytes.checked_add(SystemContractStatePrefix::SIZE))
                .and_then(|total_bytes| total_bytes.checked_next_multiple_of(u128::SIZE))
                .and_then(|total_bytes| {
                    total_bytes.checked_add(system_contract_state.contents.len())
                })
                .ok_or(StorageItemError::StorageItemTooLarge)?;
        }

        Ok(total_bytes)
    }

    const fn prefix_size() -> usize {
        // 4 lengths of header/block/mmr/num system contracts states
        const PREFIX_SIZE: usize = size_of::<u32>() * 4;
        const {
            // Ensure always aligned to `u128`
            assert!(PREFIX_SIZE == size_of::<u128>());
        }
        PREFIX_SIZE
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
        let total_bytes = self.total_bytes()? as usize;

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
                .split_off_mut(..Self::prefix_size())
                .expect("Total length checked above; qed");
            let (header_len, remainder) = prefix_bytes.split_at_mut(size_of::<u32>());
            let (body_len, remainder) = remainder.split_at_mut(size_of::<u32>());
            let (mmr_len, num_system_contract_states) = remainder.split_at_mut(size_of::<u32>());

            header_len.write_copy_of_slice(&self.header.len().to_le_bytes());
            body_len.write_copy_of_slice(&self.body.len().to_le_bytes());
            mmr_len.write_copy_of_slice(
                &u32::try_from(mmr_with_block.len())
                    .expect("Checked in `Self::total_bytes()` above; qed")
                    .to_le_bytes(),
            );
            num_system_contract_states.write_copy_of_slice(
                &u32::try_from(system_contract_states.len())
                    .expect(
                        "Each system contract state takes at least the size of its prefix out of \
                        `Self::total_bytes()`, which is `u32`; qed",
                    )
                    .to_le_bytes(),
            );

            written_len += prefix_bytes.len();
        }

        // Write content bytes
        {
            let header_bytes = buffer
                .split_off_mut(..header.len().next_multiple_of(size_of::<u128>()))
                .expect("Total length checked above; qed");

            // Sub-slice due to possible trailing alignment bytes
            header_bytes[..header.len()].write_copy_of_slice(header);
            header_bytes[header.len()..].write_filled(0);
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

            mmr_raw_bytes.write_copy_of_slice(mmr_with_block);
            written_len += mmr_raw_bytes.len();
        }

        // Alignment padding (if needed)
        if !written_len.is_multiple_of(size_of::<u64>()) {
            let new_written_len = written_len.next_multiple_of(size_of::<u64>());
            buffer
                .split_off_mut(..(new_written_len - written_len))
                .expect("Total length checked above; qed")
                .write_filled(0);
            written_len = new_written_len;
        }

        for system_contract_state in system_contract_states {
            // Alignment padding (if needed)
            if !written_len.is_multiple_of(size_of::<u64>()) {
                let new_written_len = written_len.next_multiple_of(size_of::<u64>());
                buffer
                    .split_off_mut(..(new_written_len - written_len))
                    .expect("Total length checked above; qed")
                    .write_filled(0);
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
                    .expect("Total length checked above; qed")
                    .write_filled(0);
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

        debug_assert_eq!(
            written_len, total_bytes,
            "Total bytes must match bytes written"
        );

        Ok(written_len)
    }

    /// Body references the storage item without copying it, while header and contents of system
    /// contract states are copied, since they are kept in memory for much longer than the body
    /// and must not keep the memory of the whole storage item alive
    pub(super) fn read(item: &SharedAlignedBuffer<Align16>) -> Result<Self, StorageItemError> {
        let mut reader = item.reader();

        let &[header_len, body_len, mmr_len, num_system_contract_states] =
            reader.read_trivial_array()?;
        let header = reader.read_buffer::<Align16>(header_len)?.into_standalone();
        reader.read_to_alignment::<Align16>()?;
        let body = reader.read_buffer::<Align16>(body_len)?;

        let mmr = {
            let mmr_raw_bytes = reader.read_bytes(mmr_len)?;
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
            *unsafe { BlockMerkleMountainRange::from_bytes(&mmr_bytes) }
        };

        // Every system contract state takes at least the size of the prefix, which prevents large
        // allocations due to corrupted number of system contract states
        let mut system_contract_states = Vec::with_capacity(
            usize::try_from(num_system_contract_states)
                .unwrap_or(usize::MAX)
                .min(reader.remaining().len() / size_of::<SystemContractStatePrefix>()),
        );

        for _ in 0..num_system_contract_states {
            reader.read_to_alignment::<Align8>()?;
            let prefix = reader.read_trivial::<SystemContractStatePrefix>()?;
            reader.read_to_alignment::<Align16>()?;
            let contents = reader
                .read_buffer::<Align16>(prefix.content_len)?
                .into_standalone();

            system_contract_states.push(ContractSlotState {
                owner: prefix.owner,
                contract: prefix.contract,
                contents,
            });
        }

        Ok(Self {
            header,
            body,
            mmr_with_block: Arc::new(mmr),
            system_contract_states: Arc::from(system_contract_states),
        })
    }
}
