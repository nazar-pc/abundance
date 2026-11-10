use crate::address::Address;
use crate::block::BlockRoot;
use crate::hashes::Blake3Hash;
use crate::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use crate::transaction::{
    Gas, SerializedTransactionLengths, Transaction, TransactionHash, TransactionHeader,
    TransactionSlot,
};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use core::ptr;
use std::assert_matches;

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

/// A transaction that doesn't fit into a buffer is rejected before anything is allocated or
/// copied, in particular without trying to allocate a buffer larger than the largest capacity,
/// which panics where `usize` is 32-bit. Where `usize` is 64-bit, a payload or a seal can be longer
/// than `u32::MAX` bytes, which makes the transaction too large for a buffer, while writing it into
/// a buffer reports the part that is too large before anything is written.
///
/// Payloads and seals are views of the same zeroed allocation that is never written to, so none of
/// it is backed by memory, which makes the test cheap enough to run everywhere, including under
/// Miri. The cases share one allocation instead of each test making its own, which matters where
/// allocated memory is committed upfront, like on Windows.
#[test]
fn owned_transaction_larger_than_max_capacity_is_rejected() {
    let max_capacity = usize::try_from(OwnedAlignedBuffer::MAX_CAPACITY).unwrap();
    #[cfg(target_pointer_width = "64")]
    let allocation_size = usize::try_from(u32::MAX).unwrap() + 1;
    #[cfg(not(target_pointer_width = "64"))]
    let allocation_size = max_capacity / 2;
    let allocation = vec![0u128; allocation_size / size_of::<u128>()];

    {
        let payload = &allocation[..max_capacity / 2 / size_of::<u128>()];
        let seal = u128::slice_as_bytes(payload);
        assert!(
            size_of::<TransactionHeader>()
                + size_of::<SerializedTransactionLengths>()
                + size_of_val(payload)
                + size_of_val(seal)
                > max_capacity
        );

        assert_matches!(
            OwnedTransaction::from_parts(&header(), &[], &[], payload, seal),
            Err(OwnedTransactionError::TransactionTooLarge)
        );
    }

    #[cfg(target_pointer_width = "64")]
    {
        let payload = allocation.as_slice();
        let seal = u128::slice_as_bytes(payload);
        assert_eq!(seal.len(), usize::try_from(u32::MAX).unwrap() + 1);

        assert_matches!(
            OwnedTransaction::from_parts(&header(), &[], &[], payload, &[]),
            Err(OwnedTransactionError::TransactionTooLarge)
        );
        assert_matches!(
            OwnedTransaction::from_parts(&header(), &[], &[], &[], seal),
            Err(OwnedTransactionError::TransactionTooLarge)
        );

        let mut buffer = OwnedAlignedBuffer::with_capacity(0);
        assert_matches!(
            OwnedTransaction::from_parts_into(&header(), &[], &[], payload, &[], &mut buffer),
            Err(OwnedTransactionError::PayloadTooLarge)
        );
        assert_matches!(
            OwnedTransaction::from_parts_into(&header(), &[], &[], &[], seal, &mut buffer),
            Err(OwnedTransactionError::SealTooLarge)
        );
        assert!(buffer.is_empty());
    }
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
    assert_matches!(
        OwnedTransaction::from_buffer(truncated),
        Err(OwnedTransactionError::InvalidTransaction)
    );

    let mut extended = bytes.to_vec();
    extended.extend_from_slice(&[0; 16]);
    let extended = SharedAlignedBuffer::from_bytes(&extended);
    let result = OwnedTransaction::from_buffer(extended);
    let Err(OwnedTransactionError::UnexpectedNumberOfBytes { actual, expected }) = result else {
        panic!("Unexpected result {result:?}");
    };
    assert_eq!((actual, expected), (size + 16, size));

    // Lengths after the header with padding that is not zero, then with a payload length that is
    // not a multiple of `u128` size
    let mut invalid_padding = OwnedAlignedBuffer::from_bytes(bytes);
    let (lengths, _remainder) = SerializedTransactionLengths::from_bytes_mut(
        &mut invalid_padding.as_mut_slice()[size_of::<TransactionHeader>()..],
    )
    .unwrap();
    lengths.padding = [0, 0, 0, 1];
    assert_matches!(
        OwnedTransaction::from_buffer(invalid_padding.into_shared()),
        Err(OwnedTransactionError::InvalidTransaction)
    );

    let mut invalid_payload = OwnedAlignedBuffer::from_bytes(bytes);
    let (lengths, _remainder) = SerializedTransactionLengths::from_bytes_mut(
        &mut invalid_payload.as_mut_slice()[size_of::<TransactionHeader>()..],
    )
    .unwrap();
    lengths.payload += 1;
    assert_matches!(
        OwnedTransaction::from_buffer(invalid_payload.into_shared()),
        Err(OwnedTransactionError::InvalidTransaction)
    );
}

#[test]
fn transaction_to_owned_copies_only_transaction_bytes() {
    let owned_transaction =
        OwnedTransaction::from_parts(&header(), &slots(1, 100), &slots(2, 200), &[1, 2], &[3; 5])
            .unwrap();
    let bytes = owned_transaction.buffer().as_slice();
    let mut extended = OwnedAlignedBuffer::from_bytes(bytes);
    assert!(extended.append(&[4; 16]));

    let (transaction, remainder) = Transaction::try_from_bytes(extended.as_slice()).unwrap();
    assert_eq!(remainder, [4; 16]);

    assert_eq!(transaction.to_owned().unwrap().buffer().as_slice(), bytes);
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

#[test]
fn transaction_hash_is_hash_of_serialized_transaction() {
    let transactions = [
        OwnedTransaction::from_parts(&header(), &[], &[], &[], &[]).unwrap(),
        OwnedTransaction::from_parts(&header(), &slots(2, 100), &slots(1, 200), &[1, 2], &[3; 5])
            .unwrap(),
    ];

    for owned_transaction in transactions {
        assert_eq!(
            **owned_transaction.transaction().hash(),
            Blake3Hash::from(blake3::hash(owned_transaction.buffer().as_slice()))
        );
    }
}

#[test]
fn owned_transaction_constructors_cache_hash() {
    let read_slots = slots(2, 100);
    let write_slots = slots(1, 200);
    let from_parts =
        OwnedTransaction::from_parts(&header(), &read_slots, &write_slots, &[1, 2], &[3; 5])
            .unwrap();
    let bytes = from_parts.buffer().as_slice();
    let mut extended = OwnedAlignedBuffer::from_bytes(bytes);
    assert!(extended.append(&[4; 16]));
    let (transaction, _remainder) = Transaction::try_from_bytes(extended.as_slice()).unwrap();
    let expected_hash = TransactionHash::from(Blake3Hash::from(blake3::hash(bytes)));

    let owned_transactions = [
        OwnedTransaction::from_buffer(SharedAlignedBuffer::from_bytes(bytes)).unwrap(),
        transaction.to_owned().unwrap(),
        from_parts.clone(),
    ];

    for owned_transaction in owned_transactions {
        let transaction = owned_transaction.transaction();
        assert_eq!(owned_transaction.buffer().as_slice(), bytes);
        assert_eq!(transaction.read_slots, read_slots);
        assert_eq!(transaction.write_slots, write_slots);
        assert_eq!(transaction.payload, [1, 2]);
        assert_eq!(transaction.seal, [3; 5]);

        assert_eq!(owned_transaction.hash(), expected_hash);
        assert_eq!(*transaction.hash(), expected_hash);

        // The hash is computed once and shared by clones
        let clone = owned_transaction.clone();
        assert!(ptr::eq(
            &raw const *clone.transaction().hash(),
            &raw const *transaction.hash()
        ));
        assert!(ptr::eq(
            &raw const *transaction.hash(),
            &raw const *transaction.hash()
        ));
    }
}

#[test]
fn transaction_hash_commits_to_field_boundaries() {
    let header = header();
    let slot = TransactionSlot {
        owner: Address::from(100),
        contract: Address::SYSTEM_CODE,
    };
    let mut payload_and_seal = 1u128.to_ne_bytes().to_vec();
    payload_and_seal.extend_from_slice(&[2, 3, 4]);
    let slot_as_payload = [u128::from(slot.owner), u128::from(slot.contract)];
    let pairs = [
        // Payload and seal
        (
            OwnedTransaction::from_parts(&header, &[], &[], &[1], &[2, 3, 4]).unwrap(),
            OwnedTransaction::from_parts(&header, &[], &[], &[], &payload_and_seal).unwrap(),
        ),
        // Read and write slots
        (
            OwnedTransaction::from_parts(&header, &[slot], &[], &[], &[]).unwrap(),
            OwnedTransaction::from_parts(&header, &[], &[slot], &[], &[]).unwrap(),
        ),
        // Write slots and payload
        (
            OwnedTransaction::from_parts(&header, &[], &[slot], &[], &[]).unwrap(),
            OwnedTransaction::from_parts(&header, &[], &[], &slot_as_payload, &[]).unwrap(),
        ),
    ];
    let fields_offset = size_of::<TransactionHeader>() + size_of::<SerializedTransactionLengths>();

    for (transaction_a, transaction_b) in pairs {
        let bytes_a = transaction_a.buffer().as_slice();
        let bytes_b = transaction_b.buffer().as_slice();
        assert_eq!(bytes_a[fields_offset..], bytes_b[fields_offset..]);
        assert_ne!(bytes_a, bytes_b);

        assert_eq!(
            **transaction_a.transaction().hash(),
            Blake3Hash::from(blake3::hash(bytes_a))
        );
        assert_eq!(
            **transaction_b.transaction().hash(),
            Blake3Hash::from(blake3::hash(bytes_b))
        );
        assert_ne!(transaction_a.hash(), transaction_b.hash());
    }
}
