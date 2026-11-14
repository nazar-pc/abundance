use ab_core_primitives::address::Address;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::transaction::owned::OwnedTransaction;
use ab_core_primitives::transaction::{Gas, TransactionHeader};
use ab_transaction_pool::{TransactionAddError, TransactionPool, TransactionPoolLimits};
use std::num::{NonZeroU8, NonZeroU64, NonZeroUsize};
use std::{assert_matches, iter};

#[test]
fn transactions_are_indexed_by_their_hash() {
    let block_root = BlockRoot::default();
    let mut pool = TransactionPool::new(
        NonZeroU64::new(10).unwrap(),
        NonZeroU8::new(1).unwrap(),
        TransactionPoolLimits {
            count: NonZeroUsize::new(10).unwrap(),
            size: NonZeroUsize::new(1024).unwrap(),
        },
    );
    pool.add_best_block(BlockNumber::ZERO, block_root);

    let tx = OwnedTransaction::from_parts(
        &TransactionHeader {
            version: TransactionHeader::TRANSACTION_VERSION,
            block_root,
            gas_limit: Gas::default(),
            contract: Address::SYSTEM_CODE,
        },
        &[],
        &[],
        &[1],
        &[2, 3, 4],
    )
    .unwrap();
    let tx_hash = *tx.transaction().hash();

    pool.add(tx.clone()).unwrap();
    assert!(pool.contains(&tx_hash));
    assert_matches!(pool.add(tx), Err(TransactionAddError::AlreadyExists));

    pool.remove(iter::once(&tx_hash));
    assert!(!pool.contains(&tx_hash));
    assert_eq!(pool.iter().len(), 0);
}
