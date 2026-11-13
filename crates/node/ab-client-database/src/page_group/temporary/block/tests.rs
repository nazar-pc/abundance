use crate::page_group::temporary::block::StorageItemTemporaryBlock;
use crate::storage_backend::AlignedPage;
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::write_to_aligned_pages;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::ContractSlotState;
use ab_core_primitives::address::Address;
use ab_core_primitives::trees::BlockMerkleMountainRange;
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
        let total_bytes = storage_item.total_bytes();
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
        let total_bytes = storage_item.total_bytes();
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
