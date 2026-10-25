use crate::address::Address;
use crate::block::BlockRoot;
use crate::block::body::owned::OwnedLeafShardBody;
use crate::block::body::{BeaconChainBody, BlockBody};
use crate::shard::RealShardKind;
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_aligned_buffer::SharedAlignedBuffer;
use core::ptr;
use std::iter;

fn transaction_header() -> TransactionHeader {
    TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: Address::SYSTEM_CODE,
    }
}

/// A transaction that ends 3 bytes after a 16-byte boundary, so padding follows it
fn transaction() -> OwnedTransaction {
    OwnedTransaction::from_parts(&transaction_header(), &[], &[], &[1], &[2, 3, 4]).unwrap()
}

/// A copy of `bytes` that starts `offset` bytes after a 16-byte boundary
fn bytes_at(bytes: &[u8], offset: usize) -> SharedAlignedBuffer {
    let mut buffer = vec![0; offset];
    buffer.extend_from_slice(bytes);
    SharedAlignedBuffer::from_bytes(&buffer)
}

#[test]
fn owned_leaf_shard_body_multiple_transactions() {
    let header = transaction_header();
    let slot = TransactionSlot {
        owner: Address::from(100),
        contract: Address::SYSTEM_CODE,
    };
    // Different sizes and contents, with seals that require padding after them
    let transactions = [
        OwnedTransaction::from_parts(&header, &[slot], &[], &[1], &[2, 3, 4]).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[slot, slot], &[5, 6], &[7]).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[], &[], &[8, 9]).unwrap(),
    ];

    let mut builder = OwnedLeafShardBody::init(iter::empty()).unwrap();
    for transaction in &transactions {
        builder.add_transaction(transaction).unwrap();
    }
    let body = builder.finish();

    let body_transactions = body.body().transactions();
    assert_eq!(body_transactions.len(), transactions.len());
    for (body_transaction, transaction) in body_transactions.iter().zip(&transactions) {
        assert_eq!(
            body_transaction.encoded_size(),
            usize::try_from(transaction.buffer().len()).unwrap()
        );
        assert_eq!(
            body_transaction.write_slots,
            transaction.transaction().write_slots
        );
        assert_eq!(body_transaction.payload, transaction.transaction().payload);
        assert_eq!(body_transaction.seal, transaction.transaction().seal);
    }
}

#[test]
fn owned_leaf_shard_body_aligns_transactions_to_max_alignment() {
    // The fields before the first transaction take 5 bytes, so both transactions need padding
    // before them
    let transaction = transaction();
    let mut builder = OwnedLeafShardBody::init(iter::empty()).unwrap();
    builder.add_transaction(&transaction).unwrap();
    builder.add_transaction(&transaction).unwrap();
    let body = builder.finish();

    let buffer_start = body.buffer().as_ptr().addr();
    let body_transactions = body.body().transactions();
    assert_eq!(body_transactions.len(), 2);
    for body_transaction in body_transactions.iter() {
        let offset = ptr::from_ref(body_transaction.header).addr() - buffer_start;
        assert!(offset.is_multiple_of(16), "Offset {offset}");
        assert_eq!(body_transaction.payload, [1]);
        assert_eq!(body_transaction.seal, [2, 3, 4]);
    }
}

#[test]
fn beacon_chain_body_bytes_must_be_4_bytes_aligned() {
    for offset in 0..8_usize {
        // An empty body at `offset`: zero numbers of PoT checkpoints, own segment roots and
        // intermediate shard blocks, then zero padding up to the next 8-byte boundary in memory
        let len = (offset + 7).next_multiple_of(8) - offset;
        let bytes = bytes_at(&vec![0; len], offset);
        let bytes = &bytes.as_slice()[offset..];

        // The number of PoT checkpoints is read as an aligned `u32`, and the padding adapts to
        // the position of the rest
        let expected = (offset % 4 == 0).then_some(0);
        assert_eq!(
            BeaconChainBody::try_from_bytes(bytes).map(|(_body, remainder)| remainder.len()),
            expected
        );
        assert_eq!(
            BeaconChainBody::try_from_bytes_unchecked(bytes)
                .map(|(_body, remainder)| remainder.len()),
            expected
        );
        assert_eq!(
            BlockBody::try_from_bytes(bytes, RealShardKind::BeaconChain)
                .map(|(_body, remainder)| remainder.len()),
            expected
        );
    }
}
