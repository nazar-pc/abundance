use crate::page_group::temporary::block::{StorageItemTemporaryBlock, SystemContractStatePrefix};
use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::write_to_aligned_pages;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::trees::BlockMerkleMountainRange;
use ab_io_type::trivial_type::TrivialType;
use std::{assert_matches, iter};
use triomphe::Arc;

// Lengths around alignment boundaries, a smaller set under Miri, where exhaustive checks take too
// long
const HEADER_LENS: &[u32] = if cfg!(miri) {
    &[1]
} else {
    &[0, 1, 15, 16, 17]
};
const BODY_LENS: &[u32] = if cfg!(miri) {
    &[0, 8]
} else {
    &[0, 1, 4, 7, 8, 9, 15, 16, 17, 24, 31, 32]
};
const CONTENTS_LENS: &[&[u32]] = if cfg!(miri) {
    &[&[], &[1], &[0, 17]]
} else {
    &[
        &[],
        &[0],
        &[1],
        &[8],
        &[16],
        &[17],
        &[0, 5],
        &[5, 0, 33],
        &[40, 8, 1],
    ]
};

/// Bytes taken by a system contract state in addition to its contents when it starts at a multiple
/// of 16 bytes
const STATE_OVERHEAD: u32 = SystemContractStatePrefix::SIZE.next_multiple_of(16);
/// Contents length of all but the last system contract state of a large storage item, a multiple
/// of 16
const LARGE_CONTENTS_LEN: u32 = 1024 * 1024;

fn storage_item(
    header_len: u32,
    body_len: u32,
    contents_lens: &[u32],
) -> StorageItemTemporaryBlock {
    let mut mmr_with_block = BlockMerkleMountainRange::new();
    assert!(mmr_with_block.add_leaf(&[1; _]));

    StorageItemTemporaryBlock {
        header: SharedAlignedBuffer::from_bytes(&vec![2; header_len as usize]),
        body: SharedAlignedBuffer::from_bytes(&vec![3; body_len as usize]),
        mmr_with_block: Arc::new(mmr_with_block),
        system_contract_states: contents_lens
            .iter()
            .zip(0u8..)
            .map(|(&contents_len, index)| ContractSlotState {
                owner: Address::from(u128::from(index) * 2),
                contract: Address::from(u128::from(index) * 2 + 1),
                contents: SharedAlignedBuffer::from_bytes(&vec![
                    u8::MAX - index;
                    contents_len as usize
                ]),
            })
            .collect(),
    }
}

fn storage_items() -> impl Iterator<Item = StorageItemTemporaryBlock> {
    HEADER_LENS.iter().flat_map(|&header_len| {
        BODY_LENS.iter().flat_map(move |&body_len| {
            CONTENTS_LENS
                .iter()
                .map(move |contents_lens| storage_item(header_len, body_len, contents_lens))
        })
    })
}

/// Storage item that takes `u32::MAX + extra_bytes` bytes, all system contract states except the
/// last one share the same contents to keep memory usage low
fn large_storage_item(extra_bytes: u32) -> StorageItemTemporaryBlock {
    let mut storage_item = storage_item(0, 0, &[]);
    // System contract states start where the storage item without them ends, a multiple of 8
    // bytes. Make it a multiple of 16 bytes for `STATE_OVERHEAD` to apply to every state.
    if !storage_item.total_bytes().unwrap().is_multiple_of(16) {
        storage_item.body = SharedAlignedBuffer::from_bytes(&[0; 8]);
    }

    let states_len = u64::from(u32::MAX) + u64::from(extra_bytes)
        - u64::from(storage_item.total_bytes().unwrap());
    let state_len = u64::from(STATE_OVERHEAD + LARGE_CONTENTS_LEN);
    let num_states = (states_len - u64::from(STATE_OVERHEAD)) / state_len;
    let last_contents_len = states_len - num_states * state_len - u64::from(STATE_OVERHEAD);

    let state = ContractSlotState {
        owner: Address::NULL,
        contract: Address::SYSTEM_STATE,
        contents: SharedAlignedBuffer::from_bytes(&vec![0; LARGE_CONTENTS_LEN as usize]),
    };
    let last_state = ContractSlotState {
        contents: SharedAlignedBuffer::from_bytes(&vec![
            0;
            usize::try_from(last_contents_len).unwrap()
        ]),
        ..state.clone()
    };
    storage_item.system_contract_states =
        iter::repeat_n(state, usize::try_from(num_states).unwrap())
            .chain([last_state])
            .collect();

    storage_item
}

fn assert_same_block(
    read_storage_item: &StorageItemTemporaryBlock,
    storage_item: &StorageItemTemporaryBlock,
) {
    assert_eq!(
        read_storage_item.header.as_slice(),
        storage_item.header.as_slice()
    );
    assert_eq!(
        read_storage_item.body.as_slice(),
        storage_item.body.as_slice()
    );
    assert_eq!(
        read_storage_item.mmr_with_block.as_bytes().as_slice(),
        storage_item.mmr_with_block.as_bytes().as_slice()
    );
    assert_eq!(
        read_storage_item.system_contract_states.len(),
        storage_item.system_contract_states.len()
    );
    for (read_state, state) in read_storage_item
        .system_contract_states
        .iter()
        .zip(storage_item.system_contract_states.iter())
    {
        assert_eq!(read_state.owner, state.owner);
        assert_eq!(read_state.contract, state.contract);
        assert_eq!(read_state.contents.as_slice(), state.contents.as_slice());
    }
}

#[test]
fn read_returns_written_block() {
    for storage_item in storage_items() {
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let buffer_len = total_bytes + 17;
        let (result, pages) =
            write_to_aligned_pages(buffer_len, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();

        // The caller frames storage items, so trailing bytes are ignored
        for read_len in [total_bytes, buffer_len] {
            let read_storage_item = StorageItemTemporaryBlock::read(&bytes[..read_len]).unwrap();
            assert_same_block(&read_storage_item, &storage_item);
        }
    }
}

#[test]
fn read_of_truncated_bytes_needs_more_bytes() {
    for storage_item in storage_items() {
        let total_bytes = storage_item.total_bytes().unwrap() as usize;
        let (result, pages) =
            write_to_aligned_pages(total_bytes, |buffer| storage_item.write(buffer));
        result.unwrap();
        let bytes = AlignedPage::slice_to_repr(&pages).as_flattened();
        // Without system contract states, trailing padding after MMR is not needed for reading
        let min_read_len = if storage_item.system_contract_states.is_empty() {
            size_of::<u128>()
                + (storage_item.header.len() as usize).next_multiple_of(size_of::<u128>())
                + storage_item.body.len() as usize
                + storage_item.mmr_with_block.as_bytes().len()
        } else {
            total_bytes
        };

        for read_len in 0..min_read_len {
            let result = StorageItemTemporaryBlock::read(&bytes[..read_len]);
            let Err(StorageItemError::NeedMoreBytes(missing_bytes)) = result else {
                panic!("Unexpected result for read length {read_len}: {result:?}");
            };
            assert!(
                missing_bytes > 0 && missing_bytes <= total_bytes - read_len,
                "Unexpected number of missing bytes {missing_bytes} for read length {read_len}"
            );
        }
    }
}

#[test]
fn block_of_u32_max_bytes_fits() {
    let storage_item = large_storage_item(0);

    assert_eq!(storage_item.total_bytes().unwrap(), u32::MAX);
}

#[test]
fn block_over_u32_max_bytes_is_too_large() {
    let storage_item = large_storage_item(1);

    assert_matches!(
        storage_item.total_bytes(),
        Err(StorageItemError::StorageItemTooLarge)
    );
    let (result, _pages) = write_to_aligned_pages(0, |buffer| storage_item.write(buffer));
    assert_matches!(result, Err(StorageItemError::StorageItemTooLarge));
}
