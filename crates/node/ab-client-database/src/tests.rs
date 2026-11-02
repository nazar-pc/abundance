use crate::test_utils::{
    Database, MemoryStorageBackend, highest_persisted_number, open_database, persist_block,
    persist_chain, reopen, segment_header, super_segment_header,
};
use ab_client_api::{
    BeaconChainInfo, BeaconChainInfoWrite, ChainInfo, ChainInfoWrite, ReadBlockError,
};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{LocalSegmentIndex, SuperSegmentIndex};
use futures::executor::block_on;

/// Check that blocks up to the highest persisted block number survived reopening the database
/// after the best block was at `best_number`, and that importing continues from there
fn assert_persisted_chain(database: &Database, mut chain: Vec<BlockRoot>, best_number: u64) {
    let persisted_best_number = highest_persisted_number(best_number);
    chain.truncate(usize::try_from(persisted_best_number + 1).unwrap());
    assert_eq!(&database.best_root(), chain.last().unwrap());
    for block_root in &chain {
        assert!(database.header(block_root).is_some());
        let block = block_on(database.block(block_root)).unwrap();
        assert_eq!(*block.header.header().root(), *block_root);
    }

    persist_block(
        database,
        persisted_best_number + 1,
        *chain.last().unwrap(),
        0,
    )
    .unwrap();
}

#[test]
fn segment_headers_and_blocks_survive_reopen() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];

    // Blocks are written after each segment headers item
    block_on(database.persist_segment_headers(vec![segment_header(0), segment_header(1)])).unwrap();
    chain.extend(persist_chain(&database, chain[0], 1..=10, 0));
    block_on(database.persist_segment_headers(vec![segment_header(2)])).unwrap();
    chain.extend(persist_chain(&database, chain[10], 11..=12, 0));

    let database = reopen(database, &storage_backend);

    assert_eq!(database.last_segment_header(), Some(segment_header(2)));
    for local_segment_index in 0..=2 {
        assert_eq!(
            database.get_segment_header(LocalSegmentIndex::from(local_segment_index)),
            Some(segment_header(local_segment_index))
        );
    }
    assert_persisted_chain(&database, chain, 12);
}

#[test]
fn super_segment_headers_and_blocks_survive_reopen() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];

    // Blocks are written after each super segment headers item
    block_on(
        database
            .persist_super_segment_headers(vec![super_segment_header(0), super_segment_header(1)]),
    )
    .unwrap();
    chain.extend(persist_chain(&database, chain[0], 1..=10, 0));
    assert!(block_on(database.persist_super_segment_header(super_segment_header(2))).unwrap());
    chain.extend(persist_chain(&database, chain[10], 11..=12, 0));

    let database = reopen(database, &storage_backend);

    assert_eq!(
        database.last_super_segment_header(),
        Some(super_segment_header(2))
    );
    for super_segment_index in 0..=2 {
        assert_eq!(
            database.get_super_segment_header(SuperSegmentIndex::from(super_segment_index)),
            Some(super_segment_header(super_segment_index))
        );
    }
    assert_persisted_chain(&database, chain, 12);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn fork_below_best_block_keeps_best_block() {
    // genesis - c1 .. c3 -+- c4 - c5 - c6 (best block)
    //                     \- a4 - a5 (imported below the best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=5, 0));
    let c5 = chain[5];

    let a4 = persist_block(&database, 4, chain[3], 1).unwrap();

    assert_eq!(database.best_root(), c5);
    assert!(database.header(&a4).is_some());

    // Both the best chain and the fork can still be extended
    let c6 = persist_block(&database, 6, c5, 0).unwrap();
    assert_eq!(database.best_root(), c6);
    persist_block(&database, 5, a4, 1).unwrap();
    assert_eq!(database.best_root(), c6);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn pruned_fork_tip_is_unknown() {
    // genesis - c1 - c2 -+- c3 (best block)
    //                    +- a3 (oldest fork tip, pruned)
    //                    +- b3
    //                    \- d3 (added last, exceeds `max_fork_tips` together with the best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=3, 0));

    let a3 = persist_block(&database, 3, chain[2], 1).unwrap();
    let [b3, d3] = [2, 3].map(|salt| persist_block(&database, 3, chain[2], salt).unwrap());

    assert!(database.header(&a3).is_none());
    assert!(matches!(
        block_on(database.block(&a3)),
        Err(ReadBlockError::UnknownBlockRoot)
    ));
    for block_root in [b3, d3] {
        assert!(database.header(&block_root).is_some());
        block_on(database.block(&block_root)).unwrap();
    }
    assert_eq!(database.best_root(), chain[3]);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn pruned_fork_tip_keeps_parent_with_other_children() {
    // genesis -+- c1 -+- c2
    //          |      \- d2 (added last, exceeds `max_fork_tips`)
    //          \- a1 -+- a2 (oldest fork tip, pruned)
    //                 \- b2 (keeps `a1`, then extended with `b3`, the new best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let genesis_root = database.best_root();

    let c1 = persist_block(&database, 1, genesis_root, 0).unwrap();
    let a1 = persist_block(&database, 1, genesis_root, 1).unwrap();
    persist_block(&database, 2, c1, 0).unwrap();
    let a2 = persist_block(&database, 2, a1, 1).unwrap();
    let b2 = persist_block(&database, 2, a1, 2).unwrap();
    persist_block(&database, 2, c1, 3).unwrap();

    assert!(database.header(&a2).is_none());
    assert!(database.header(&a1).is_some());
    assert_eq!(
        database
            .ancestor_header(BlockNumber::ONE, &b2)
            .map(|header| *header.header().root()),
        Some(a1)
    );

    let b3 = persist_block(&database, 3, b2, 0).unwrap();
    assert_eq!(database.best_root(), b3);
}
