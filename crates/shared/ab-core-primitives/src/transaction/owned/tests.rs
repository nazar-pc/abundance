use crate::address::Address;
use crate::block::BlockRoot;
use crate::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use crate::transaction::{Gas, Transaction, TransactionHeader, TransactionSlot};
use ab_aligned_buffer::SharedAlignedBuffer;

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
}
