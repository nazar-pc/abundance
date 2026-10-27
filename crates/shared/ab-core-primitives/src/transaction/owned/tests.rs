use crate::address::Address;
use crate::block::BlockRoot;
use crate::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use crate::transaction::{
    Gas, SerializedTransactionLengths, Transaction, TransactionHeader, TransactionSlot,
};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;

fn header() -> TransactionHeader {
    TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: Address::SYSTEM_CODE,
    }
}

fn slots(count: u128, first_owner: u128) -> Vec<TransactionSlot> {
    (first_owner..first_owner + count)
        .map(|owner| TransactionSlot {
            owner: Address::from(owner),
            contract: Address::SYSTEM_CODE,
        })
        .collect()
}

#[test]
fn owned_transaction_from_parts_different_slot_counts() {
    let header = header();

    for (num_read_slots, num_write_slots) in [(2, 0), (0, 2), (1, 3)] {
        let read_slots = slots(num_read_slots, 100);
        let write_slots = slots(num_write_slots, 200);

        let owned_transaction =
            OwnedTransaction::from_parts(&header, &read_slots, &write_slots, &[], &[]).unwrap();
        let transaction = owned_transaction.transaction();

        assert_eq!(transaction.read_slots, read_slots);
        assert_eq!(transaction.write_slots, write_slots);
        assert_eq!(
            transaction.encoded_size(),
            usize::try_from(owned_transaction.buffer().len()).unwrap()
        );
    }
}

#[test]
fn owned_transaction_payload_and_seal() {
    let header = header();
    let read_slots = slots(1, 100);
    let write_slots = slots(2, 200);
    let payload = [1, 2, 3];
    let seal = [4, 5, 6, 7, 8];

    let owned_transaction =
        OwnedTransaction::from_parts(&header, &read_slots, &write_slots, &payload, &seal).unwrap();
    let transaction = owned_transaction.transaction();

    assert_eq!(transaction.payload, payload);
    assert_eq!(transaction.seal, seal);
    assert_eq!(
        transaction.encoded_size(),
        usize::try_from(owned_transaction.buffer().len()).unwrap()
    );

    let (decoded, remainder) =
        Transaction::try_from_bytes(owned_transaction.buffer().as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(decoded.read_slots, read_slots);
    assert_eq!(decoded.write_slots, write_slots);
    assert_eq!(decoded.payload, payload);
    assert_eq!(decoded.seal, seal);
}

#[test]
fn owned_transaction_from_buffer() {
    let header = header();
    let read_slots = slots(2, 100);
    let write_slots = slots(1, 200);
    let payload = [1, 2];
    let seal = [3; 16];

    let owned_transaction =
        OwnedTransaction::from_parts(&header, &read_slots, &write_slots, &payload, &seal).unwrap();
    let bytes = owned_transaction.buffer().as_slice();
    let size = owned_transaction.buffer().len();

    let decoded = OwnedTransaction::from_buffer(owned_transaction.buffer().clone()).unwrap();
    assert_eq!(decoded.buffer().as_slice(), bytes);

    // Missing the last 16 bytes of the seal
    let truncated = SharedAlignedBuffer::from_bytes(&bytes[..bytes.len() - 16]);
    assert!(matches!(
        OwnedTransaction::from_buffer(truncated),
        Err(OwnedTransactionError::NotEnoughBytes)
    ));

    let mut extended = bytes.to_vec();
    extended.extend_from_slice(&[0; 16]);
    let extended = SharedAlignedBuffer::from_bytes(&extended);
    assert!(matches!(
        OwnedTransaction::from_buffer(extended),
        Err(OwnedTransactionError::UnexpectedNumberOfBytes { actual, expected })
            if actual == size + 16 && expected == size
    ));

    // Lengths after the header with padding that is not zero, then with a payload length that is
    // not a multiple of `u128` size
    let mut invalid_padding = OwnedAlignedBuffer::from_bytes(bytes);
    let (lengths, _remainder) = SerializedTransactionLengths::from_bytes_mut(
        &mut invalid_padding.as_mut_slice()[size_of::<TransactionHeader>()..],
    )
    .unwrap();
    lengths.padding = [0, 0, 0, 1];
    assert!(matches!(
        OwnedTransaction::from_buffer(invalid_padding.into_shared()),
        Err(OwnedTransactionError::InvalidPadding)
    ));

    let mut invalid_payload = OwnedAlignedBuffer::from_bytes(bytes);
    let (lengths, _remainder) = SerializedTransactionLengths::from_bytes_mut(
        &mut invalid_payload.as_mut_slice()[size_of::<TransactionHeader>()..],
    )
    .unwrap();
    lengths.payload += 1;
    assert!(matches!(
        OwnedTransaction::from_buffer(invalid_payload.into_shared()),
        Err(OwnedTransactionError::PayloadIsNotMultipleOfU128)
    ));
}

#[test]
fn transaction_bytes_must_be_aligned_to_max_alignment() {
    let owned_transaction =
        OwnedTransaction::from_parts(&header(), &slots(1, 100), &[], &[1, 2], &[3; 5]).unwrap();
    let bytes = owned_transaction.buffer().as_slice();

    let (transaction, remainder) = Transaction::try_from_bytes(bytes).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(transaction.payload, [1, 2]);
    assert_eq!(transaction.seal, [3; 5]);

    // 8 bytes after a 16-byte boundary
    let mut misaligned = OwnedAlignedBuffer::from_bytes(&[0; 8]);
    assert!(misaligned.append(bytes));
    assert!(Transaction::try_from_bytes(&misaligned.as_slice()[8..]).is_none());
}
