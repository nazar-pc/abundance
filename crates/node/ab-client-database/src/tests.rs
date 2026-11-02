use crate::test_utils::{
    Database, MemoryStorageBackend, highest_persisted_number, open_database, persist_block,
    persist_chain, reopen, segment_header, super_segment_header,
};
use ab_client_api::{BeaconChainInfo, BeaconChainInfoWrite, ChainInfo, ChainInfoWrite};
use ab_core_primitives::block::BlockRoot;
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
