use crate::address::Address;
use crate::block::BlockRoot;
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader, TransactionSlot};

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
