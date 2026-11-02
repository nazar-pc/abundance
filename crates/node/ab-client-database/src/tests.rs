use crate::test_utils::{
    Database, DatabaseOptions, MemoryStorageBackend, beacon_chain_block, block_confirmation_depth,
    database_options, genesis_block_builder, highest_persisted_number, open_database,
    open_database_with_options, persist_block, persist_chain, reopen, segment_header,
    super_segment_header,
};
use crate::{ClientDatabaseBlock, ClientDatabaseError};
use ab_client_api::{
    BeaconChainInfo, BeaconChainInfoWrite, ChainInfo, ChainInfoWrite, PersistBlockError,
    ReadBlockError,
};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{LocalSegmentIndex, SuperSegmentIndex};
use futures::executor::block_on;
use std::num::NonZeroUsize;

/// Check that blocks up to the highest persisted block number survived reopening the database
/// after the best block was at `best_number`, and that importing continues from there
fn assert_persisted_chain(database: &Database, mut chain: Vec<BlockRoot>, best_number: u64) {
    let persisted_best_number = highest_persisted_number(database, best_number);
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

/// Root of the block at `number` on the chain ending at `descendant_root`
fn ancestor_root(
    database: &Database,
    number: u64,
    descendant_root: &BlockRoot,
) -> Option<BlockRoot> {
    database
        .ancestor_header(BlockNumber::from(number), descendant_root)
        .map(|header| *header.header().root())
}

/// Check that `chain` (block roots from genesis to the best block) is the canonical chain
fn assert_best_chain(database: &Database, chain: &[BlockRoot]) {
    let best_root = database.best_root();
    assert_eq!(chain.last(), Some(&best_root));
    for (number, block_root) in (0..).zip(chain) {
        assert_eq!(
            ancestor_root(database, number, &best_root),
            Some(*block_root)
        );
    }
}

/// Check that the best block is at `best_number`, that every block offset holds its ancestor at
/// fork offset `0`, that block offsets from `block_confirmation_depth - 1` have no forks, that only
/// those ancestors are confirmed and that all of them can be read
fn assert_consistent(database: &Database, best_number: u64) {
    let block_confirmation_depth = block_confirmation_depth(database);
    let mut canonical_roots = Vec::new();
    {
        let state = database.inner.state.read_blocking();
        let blocks = &state.data.blocks;
        assert_eq!(blocks.len(), usize::try_from(best_number + 1).unwrap());

        let mut expected_root = state.best_tip().root;
        for (block_offset, (number, block_forks)) in (0..).zip((0..=best_number).rev().zip(blocks))
        {
            let (canonical_block, other_blocks) = block_forks.split_first().unwrap();
            if block_offset + 1 >= block_confirmation_depth {
                assert!(
                    other_blocks.is_empty(),
                    "Forks at block offset {block_offset}"
                );
            }
            let header = canonical_block.header().header();
            assert_eq!(header.prefix.number, BlockNumber::from(number));
            assert_eq!(*header.root(), expected_root);
            for block in other_blocks {
                assert_eq!(
                    block.header().header().prefix.number,
                    BlockNumber::from(number)
                );
                #[expect(
                    clippy::rest_pattern_accessible_field,
                    reason = "Do not need other fields"
                )]
                let confirmed = matches!(block, ClientDatabaseBlock::PersistedConfirmed { .. });
                assert!(!confirmed);
            }

            canonical_roots.push(expected_root);
            expected_root = header.prefix.parent_root;
        }
    }

    for block_root in &canonical_roots {
        let block = block_on(database.block(block_root)).unwrap();
        assert_eq!(*block.header.header().root(), *block_root);
    }
}

/// Extend the best chain with `num_blocks` blocks, then reopen the database and check it with
/// [`assert_consistent()`]
fn extend_and_reopen(
    database: Database,
    storage_backend: &MemoryStorageBackend,
    num_blocks: u64,
    salt: u64,
) -> Database {
    let best_header = database.best_header();
    let best_header = best_header.header();
    let best_number = u64::from(best_header.prefix.number);
    let new_best_number = best_number + num_blocks;
    persist_chain(
        &database,
        *best_header.root(),
        best_number + 1..=new_best_number,
        salt,
    );
    assert_consistent(&database, new_best_number);

    let database = reopen(database, storage_backend);
    assert_consistent(
        &database,
        highest_persisted_number(&database, new_best_number),
    );

    database
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

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn ancestor_header_includes_descendant() {
    // genesis - c1 - c2 -+- c3 - c4 -+- c5 (best block)
    //                    \- b3       \- a5
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=5, 0));

    // Levels without forks, at and below the best block
    for (number, block_root) in [(5, chain[5]), (3, chain[3])] {
        assert_eq!(
            ancestor_root(&database, number, &block_root),
            Some(block_root)
        );
    }

    // Levels with forks `a5` and `b3`, at and below the best block
    for (number, parent_root, block_root) in [(5, chain[4], chain[5]), (3, chain[2], chain[3])] {
        let fork_root = persist_block(&database, number, parent_root, 1).unwrap();
        for descendant_root in [block_root, fork_root] {
            assert_eq!(
                ancestor_root(&database, number, &descendant_root),
                Some(descendant_root)
            );
        }
    }
    assert_eq!(database.best_root(), chain[5]);

    assert_eq!(ancestor_root(&database, 4, &chain[3]), None);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reorgs_move_new_best_chain_to_fork_offset_zero() {
    // genesis - c1 .. c(fork_number) -+- c(fork_number + 1) .. c6 - c7 - c8 (best block again)
    //                                 \- a(fork_number + 1) .. a7 (best block first)
    //
    // Fork points `fork_number` from right below the best block to the deepest one that still
    // allows reorging back, which replaces `a(fork_number + 1) .. a7`, that is
    // `block_confirmation_depth - 1` blocks
    let block_confirmation_depth =
        u64::from(database_options(&MemoryStorageBackend::new()).block_confirmation_depth);
    let deepest_fork_number = usize::try_from(7 - (block_confirmation_depth - 1)).unwrap();
    for fork_number in deepest_fork_number..=5 {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=6, 0));

        // Fork reaching the best block number and then overtaking it
        let mut fork_chain = chain[..=fork_number].to_vec();
        fork_chain.extend(persist_chain(
            &database,
            chain[fork_number],
            u64::try_from(fork_number).unwrap() + 1..=7,
            1,
        ));
        assert_best_chain(&database, &fork_chain);

        // Reorg back to the original chain
        chain.extend(persist_chain(&database, chain[6], 7..=8, 0));
        assert_best_chain(&database, &chain);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_moves_best_chain_to_fork_offset_zero() {
    // genesis - c1 .. c3 -+- c4 .. c9 (best block, `c7 .. c9` are not persisted)
    //                     \- a4 (written before `c4`)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=6, 0));
    let a4 = persist_block(&database, 4, chain[3], 1).unwrap();
    // Persists `a4`, which ends up below the highest persisted block number
    chain.extend(persist_chain(&database, chain[6], 7..=9, 0));

    let database = reopen(database, &storage_backend);

    let persisted_best_number = highest_persisted_number(&database, 9);
    chain.truncate(usize::try_from(persisted_best_number + 1).unwrap());
    assert_best_chain(&database, &chain);
    assert!(database.header(&a4).is_some());
    persist_block(
        &database,
        persisted_best_number + 1,
        *chain.last().unwrap(),
        0,
    )
    .unwrap();
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn block_with_unknown_parent_is_rejected() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let genesis_root = database.best_root();
    let unknown_root = *beacon_chain_block(0, BlockRoot::default(), 1)
        .header
        .header()
        .root();

    // With only the genesis block and with more blocks in memory
    for number in 1..=2 {
        assert!(matches!(
            persist_block(&database, number, unknown_root, 0),
            Err(PersistBlockError::MissingParent)
        ));
        persist_block(&database, number, database.best_root(), 0).unwrap();
    }
    assert_eq!(
        ancestor_root(&database, 0, &database.best_root()),
        Some(genesis_root)
    );
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn fork_with_unknown_parent_is_rejected() {
    // genesis - c1 - c2 -+- c3 - c4 (best block)
    //                    +- a3 (oldest fork tip, pruned) - a4 (rejected)
    //                    +- b3
    //                    \- d3 (added last, exceeds `max_fork_tips` together with the best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=3, 0));
    let a3 = persist_block(&database, 3, chain[2], 1).unwrap();
    for salt in [2, 3] {
        persist_block(&database, 3, chain[2], salt).unwrap();
    }
    assert!(database.header(&a3).is_none());
    chain.push(persist_block(&database, 4, chain[3], 0).unwrap());

    assert!(matches!(
        persist_block(&database, 4, a3, 1),
        Err(PersistBlockError::MissingParent)
    ));
    assert_best_chain(&database, &chain);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_fork_at_every_height() {
    // genesis - c1 .. c(F-1) -+- cF .. c25 (best block)
    //                         \- aF (written before `cF`)
    //
    // For every `F`, reopened, then extended and reopened again
    for fork_number in 1..=20 {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=fork_number, 0));
        // Sibling of the best block, persisted together with it, but written before it
        let parent_root = chain[usize::try_from(fork_number - 1).unwrap()];
        persist_block(&database, fork_number, parent_root, 1).unwrap();
        let best_number = 25;
        persist_chain(
            &database,
            *chain.last().unwrap(),
            fork_number + 1..=best_number,
            0,
        );

        let database = reopen(database, &storage_backend);
        assert_consistent(&database, highest_persisted_number(&database, best_number));
        extend_and_reopen(database, &storage_backend, 10, 2);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_reorg_before_persisting() {
    // genesis - c1 .. c5 -+- c6
    //                     \- a6 .. a20 (best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=6, 0));
    // `c6` and `a6` are persisted together after the reorg, `a6` last
    let a6 = persist_block(&database, 6, chain[5], 1).unwrap();
    persist_chain(&database, a6, 7..=20, 1);

    let database = reopen(database, &storage_backend);
    assert_consistent(&database, highest_persisted_number(&database, 20));
    let database = extend_and_reopen(database, &storage_backend, 10, 2);
    extend_and_reopen(database, &storage_backend, 10, 3);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_reorg_after_persisting() {
    // genesis - c1 .. c16 -+- c17 .. c20
    //                      \- a17 .. a21 (best block, reorg of `block_confirmation_depth - 1`
    //                                     blocks)
    //
    // The fork is persisted after the original best chain or at the same time
    for fork_persisted_together in [false, true] {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        let fork_root = if fork_persisted_together {
            chain.extend(persist_chain(&database, chain[0], 1..=16, 0));
            let mut fork_root = chain[16];
            for number in 17..=18 {
                chain.push(persist_block(&database, number, *chain.last().unwrap(), 0).unwrap());
                fork_root = persist_block(&database, number, fork_root, 1).unwrap();
            }
            // The fork briefly leads with `a19` and the original best chain takes over again with
            // `c20`, `a17` is written right before `c17`
            fork_root = persist_block(&database, 19, fork_root, 1).unwrap();
            for number in 19..=20 {
                chain.push(persist_block(&database, number, *chain.last().unwrap(), 0).unwrap());
            }
            persist_block(&database, 20, fork_root, 1).unwrap()
        } else {
            chain.extend(persist_chain(&database, chain[0], 1..=20, 0));
            // `a17` and `a18` are written after `c17`, with the reorg below
            *persist_chain(&database, chain[16], 17..=20, 1)
                .last()
                .unwrap()
        };
        // Reorg
        let best_number = 21;
        persist_block(&database, best_number, fork_root, 1).unwrap();

        let database = reopen(database, &storage_backend);
        assert_consistent(&database, highest_persisted_number(&database, best_number));
        let database = extend_and_reopen(database, &storage_backend, 10, 2);
        extend_and_reopen(database, &storage_backend, 10, 3);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_late_fork() {
    // genesis - c1 .. c5 -+- c6 .. c9 - c10 .. c20 (without `reorg`, `a6` is pruned unwritten)
    //                     \- a6 - a7 .. a20 (with `reorg`, `a6` is written after `c6`)
    for reorg in [false, true] {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=9, 0));
        // `c6` was already persisted, so `a6` can only be written after it
        let a6 = persist_block(&database, 6, chain[5], 1).unwrap();
        assert_eq!(database.best_root(), chain[9]);
        let (best_root, best_number) = if reorg {
            let fork_chain = persist_chain(&database, a6, 7..=10, 1);
            (*fork_chain.last().unwrap(), 10)
        } else {
            (chain[9], 9)
        };
        persist_chain(&database, best_root, best_number + 1..=20, 2);

        let database = reopen(database, &storage_backend);
        assert_consistent(&database, highest_persisted_number(&database, 20));
        extend_and_reopen(database, &storage_backend, 10, 3);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn consecutive_reopens_with_shallow_fork() {
    // genesis - c1 .. c14 -+- c15 .. c20 (best block)
    //                      \- a15 (written before `c15`)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=15, 0));
    // `a15` is too close to the top to be confirmed on the first reopen
    persist_block(&database, 15, chain[14], 1).unwrap();
    persist_chain(&database, chain[15], 16..=20, 0);

    let database = reopen(database, &storage_backend);
    assert_consistent(&database, highest_persisted_number(&database, 20));
    let database = extend_and_reopen(database, &storage_backend, 10, 2);
    extend_and_reopen(database, &storage_backend, 10, 3);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_fork_at_reopened_top() {
    // genesis - c1 .. c(P-1) -+- cP -+- c(P+1) .. c20 (lost on the first reopen)
    //                         |      \- b(P+1) .. b20 (without `reorg`)
    //                         \- aP (written after `cP` is read) - a(P+1) .. a20 (with `reorg`)
    //
    // `P` is the highest persisted block number on the first reopen
    for reorg in [false, true] {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=20, 0));
        let persisted_best_number = highest_persisted_number(&database, 20);
        let database = reopen(database, &storage_backend);
        assert_consistent(&database, persisted_best_number);

        // `aP` is persisted after `cP` was read when reopening
        let persisted_best_index = usize::try_from(persisted_best_number).unwrap();
        let parent_root = chain[persisted_best_index - 1];
        let fork_root = persist_block(&database, persisted_best_number, parent_root, 1).unwrap();
        if reorg {
            persist_chain(&database, fork_root, persisted_best_number + 1..=20, 1);
        } else {
            let best_root = chain[persisted_best_index];
            persist_chain(&database, best_root, persisted_best_number + 1..=20, 2);
        }

        let database = reopen(database, &storage_backend);
        assert_consistent(&database, highest_persisted_number(&database, 20));
        let database = extend_and_reopen(database, &storage_backend, 10, 3);
        extend_and_reopen(database, &storage_backend, 10, 4);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_after_interrupted_writes() {
    // genesis - c1 .. c4 -+- c5 .. c12 -+- c13 .. c16
    //                     \- a5         \- b13 .. b17 (reorg, writes cut short)
    //
    // The reorg to `b17` prunes `c13 .. c16` and writes two blocks: `b13`, then `b14`
    for num_writes in 0..=2 {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=5, 0));
        // `a5` is pruned while reopening, once `c9` is read
        persist_block(&database, 5, chain[4], 1).unwrap();
        chain.extend(persist_chain(&database, chain[5], 6..=16, 0));
        let b16 = *persist_chain(&database, chain[12], 13..=16, 1)
            .last()
            .unwrap();

        storage_backend.limit_writes(Some(num_writes));
        persist_block(&database, 17, b16, 1).unwrap();
        storage_backend.limit_writes(None);

        // The new highest block number only reaches the disk with the second write
        let persisted_best_number = if num_writes < 2 {
            highest_persisted_number(&database, 16)
        } else {
            highest_persisted_number(&database, 17)
        };
        let database = reopen(database, &storage_backend);
        assert_consistent(&database, persisted_best_number);
        extend_and_reopen(database, &storage_backend, 10, 2);
    }
}

/// Persist a canonical chain (`c`) and a fork (`a`) that split after `c3`: for every block number
/// in `4..=interleave_until` a canonical block and then a fork block, after that only fork blocks
/// up to `fork_until`. Returns roots of the canonical chain from genesis and the result of
/// persisting the last fork block.
fn interleaved_fork(
    database: &Database,
    interleave_until: u64,
    fork_until: u64,
) -> (Vec<BlockRoot>, Result<BlockRoot, PersistBlockError>) {
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(database, chain[0], 1..=3, 0));
    let mut fork_root = chain[3];
    let mut result = Ok(fork_root);
    for number in 4..=fork_until {
        if number <= interleave_until {
            chain.push(persist_block(database, number, *chain.last().unwrap(), 0).unwrap());
        }
        result = persist_block(database, number, fork_root, 1);
        match &result {
            Ok(block_root) => fork_root = *block_root,
            Err(_) => break,
        }
    }

    (chain, result)
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reorg_of_confirmation_depth_minus_one_blocks() {
    // genesis - c1 .. c3 -+- c4 .. c7
    //                     \- a4 .. a8 (best block)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let block_confirmation_depth = block_confirmation_depth(&database);
    let (_chain, result) = interleaved_fork(
        &database,
        3 + block_confirmation_depth - 1,
        3 + block_confirmation_depth,
    );
    assert_eq!(database.best_root(), result.unwrap());
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reorgs_of_confirmation_depth_blocks_and_deeper_are_rejected() {
    // genesis - c1 .. c3 -+- c4 .. c(3 + block_confirmation_depth + extra) (best block)
    //                     \- a4 .. (pruned with `a4` once `c(3 + block_confirmation_depth)` is the
    //                            best block)
    //
    // `extra` is `0` for a reorg of `block_confirmation_depth` blocks and `1` for a reorg of
    // `block_confirmation_depth + 1` blocks
    for extra in [0, 1] {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database(&storage_backend);
        let block_confirmation_depth = block_confirmation_depth(&database);
        let (chain, result) = interleaved_fork(
            &database,
            3 + block_confirmation_depth + extra,
            3 + block_confirmation_depth + extra + 1,
        );
        assert!(result.is_err(), "{extra}: {result:?}");
        assert_best_chain(&database, &chain);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn forks_are_only_accepted_above_fork_pruning_block_offset() {
    // genesis - c1 .. c3 -+- c4 -+- c5 .. c8 (best block)
    //                     |      \- b5 (block offset `block_confirmation_depth - 2`) - b6 .. b9
    //                     |          (best block, reorg)
    //                     \- a4 (block offset `block_confirmation_depth - 1`, rejected)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let block_confirmation_depth = block_confirmation_depth(&database);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(
        &database,
        chain[0],
        1..=3 + block_confirmation_depth,
        0,
    ));

    assert!(matches!(
        persist_block(&database, 4, chain[3], 1),
        Err(PersistBlockError::OutsideAcceptableRange)
    ));
    assert_best_chain(&database, &chain);

    let mut fork_chain = chain[..=4].to_vec();
    fork_chain.extend(persist_chain(
        &database,
        chain[4],
        5..=4 + block_confirmation_depth,
        2,
    ));
    assert_best_chain(&database, &fork_chain);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn forks_at_fork_pruning_block_offset_are_pruned_with_descendants() {
    // genesis - c1 -+- c2 .. c4 -+- c5 (best block) - c6 (new best block)
    //               |            \- b5 (kept) - b6
    //               \- a2 -+- a3 - a4 (pruned with `a2`) - a5 (rejected)
    //                      \- d3 (pruned with `a2`)
    //
    // `max_fork_tips` keeps all four fork tips until `a2` is pruned
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database_with_options(DatabaseOptions {
        max_fork_tips: NonZeroUsize::new(4).unwrap(),
        ..database_options(&storage_backend)
    })
    .unwrap();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=5, 0));
    let fork_chain = persist_chain(&database, chain[1], 2..=4, 1);
    let d3 = persist_block(&database, 3, fork_chain[0], 2).unwrap();
    let b5 = persist_block(&database, 5, chain[4], 3).unwrap();
    for block_root in fork_chain.iter().chain([&d3]) {
        assert!(database.header(block_root).is_some());
    }

    chain.push(persist_block(&database, 6, chain[5], 0).unwrap());

    for block_root in fork_chain.iter().chain([&d3]) {
        assert!(database.header(block_root).is_none());
    }
    assert!(matches!(
        persist_block(&database, 5, fork_chain[2], 1),
        Err(PersistBlockError::MissingParent)
    ));
    assert!(database.header(&b5).is_some());
    persist_block(&database, 6, b5, 3).unwrap();
    assert_best_chain(&database, &chain);
}

#[test]
fn block_details_are_dropped_from_memory_at_block_confirmation_depth_plus_one() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let block_confirmation_depth = block_confirmation_depth(&database);
    let best_number = 3 + block_confirmation_depth;
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=best_number, 0));

    for (number, block_root) in (0..).zip(&chain) {
        let block_offset = best_number - number;
        assert_eq!(
            database.header_with_details(block_root).is_some(),
            block_offset <= block_confirmation_depth,
            "{block_offset}"
        );
        assert!(database.header(block_root).is_some(), "{block_offset}");
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn reopen_prunes_forks_at_fork_pruning_block_offset() {
    // genesis - c1 .. c4 -+- c5 -+- c6 .. c12 (`c10 .. c12` are not persisted)
    //                     |      \- b6 (block offset `block_confirmation_depth - 2` after
    //                     |          reopening) - b7 .. b10
    //                     +- a5 (written before `c5`, pruned at `c9` and on reopen) - a6 (rejected)
    //                     \- d5 (block offset `block_confirmation_depth - 1` after reopening,
    //                         rejected)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let block_confirmation_depth = block_confirmation_depth(&database);
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=6, 0));
    let a5 = persist_block(&database, 5, chain[4], 1).unwrap();
    chain.extend(persist_chain(&database, chain[6], 7..=12, 0));
    assert!(database.header(&a5).is_none());

    let database = reopen(database, &storage_backend);
    let persisted_best_number = highest_persisted_number(&database, 12);
    assert_consistent(&database, persisted_best_number);
    chain.truncate(usize::try_from(persisted_best_number + 1).unwrap());
    assert!(database.header(&a5).is_none());
    assert!(matches!(
        persist_block(&database, 6, a5, 1),
        Err(PersistBlockError::MissingParent)
    ));
    assert!(matches!(
        persist_block(
            &database,
            persisted_best_number + 1 - block_confirmation_depth,
            chain[4],
            2
        ),
        Err(PersistBlockError::OutsideAcceptableRange)
    ));

    let mut fork_chain = chain[..=5].to_vec();
    fork_chain.extend(persist_chain(
        &database,
        chain[5],
        6..=persisted_best_number + 1,
        3,
    ));
    assert_best_chain(&database, &fork_chain);
}

#[test]
fn block_confirmation_depth_must_be_at_least_two() {
    let storage_backend = MemoryStorageBackend::new();
    // Valid for the smallest valid block confirmation depth
    let options_with_depth = |block_confirmation_depth| DatabaseOptions {
        block_confirmation_depth: BlockNumber::from(block_confirmation_depth),
        soft_confirmation_depth: BlockNumber::ZERO,
        ..database_options(&storage_backend)
    };

    for block_confirmation_depth in [0, 1] {
        assert!(
            matches!(
                open_database_with_options(options_with_depth(block_confirmation_depth)),
                Err(ClientDatabaseError::InvalidBlockConfirmationDepth)
            ),
            "{block_confirmation_depth}"
        );
    }
    open_database_with_options(options_with_depth(2)).unwrap();
}

#[test]
fn soft_confirmation_depth_must_be_smaller_than_block_confirmation_depth() {
    let storage_backend = MemoryStorageBackend::new();
    let options = database_options(&storage_backend);
    let block_confirmation_depth = options.block_confirmation_depth;

    assert!(matches!(
        open_database_with_options(DatabaseOptions {
            soft_confirmation_depth: block_confirmation_depth,
            ..options.clone()
        }),
        Err(ClientDatabaseError::InvalidSoftConfirmationDepth)
    ));
    open_database_with_options(DatabaseOptions {
        soft_confirmation_depth: block_confirmation_depth - BlockNumber::ONE,
        ..options
    })
    .unwrap();
}

#[test]
fn default_max_fork_tip_distance_is_accepted_with_small_block_confirmation_depths() {
    // For all of these the default is larger than `block_confirmation_depth - 2`, from where
    // values have no extra effect
    for block_confirmation_depth in 2..=6 {
        open_database_with_options(DatabaseOptions {
            block_confirmation_depth: BlockNumber::from(block_confirmation_depth),
            soft_confirmation_depth: BlockNumber::ONE,
            genesis_block_builder,
            storage_backend: MemoryStorageBackend::new(),
            ..
        })
        .unwrap();
    }
}

/// Block roots by block offset and fork offset, and fork tip roots
fn block_and_fork_tip_roots(database: &Database) -> (Vec<Vec<BlockRoot>>, Vec<BlockRoot>) {
    let state = database.inner.state.read_blocking();
    let block_roots = state
        .data
        .blocks
        .iter()
        .map(|block_forks| {
            block_forks
                .iter()
                .map(|block| *block.header().header().root())
                .collect()
        })
        .collect();
    let fork_tip_roots = state
        .data
        .fork_tips
        .iter()
        .map(|fork_tip| fork_tip.root)
        .collect();

    (block_roots, fork_tip_roots)
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn max_fork_tip_distance_from_block_confirmation_depth_minus_two_up_has_the_same_effect() {
    // genesis - c1 .. c12 (best block), with the test `block_confirmation_depth` of 5 each best
    // block `c(number)` from `c4` on is followed by two fork blocks:
    //
    // c(number - 4) -+- c(number - 3) .. c(number - 1) -+- c(number) (best block)
    //                |                                  \- b(number)
    //                \- a(number - 3) (block offset `block_confirmation_depth - 2`, the deepest
    //                   accepted one)
    let block_confirmation_depth =
        database_options(&MemoryStorageBackend::new()).block_confirmation_depth;
    let deepest_fork_block_offset = u64::from(block_confirmation_depth) - 2;
    let roots_after_each_best_block = |max_fork_tip_distance| {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database_with_options(DatabaseOptions {
            max_fork_tip_distance,
            ..database_options(&storage_backend)
        })
        .unwrap();
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(
            &database,
            chain[0],
            1..=deepest_fork_block_offset,
            0,
        ));

        (deepest_fork_block_offset + 1..=12)
            .map(|number| {
                let parent_root = *chain.last().unwrap();
                chain.push(persist_block(&database, number, parent_root, 0).unwrap());
                persist_block(&database, number, parent_root, 1).unwrap();
                let fork_number = number - deepest_fork_block_offset;
                let fork_parent_root = chain[usize::try_from(fork_number - 1).unwrap()];
                persist_block(&database, fork_number, fork_parent_root, 2).unwrap();

                block_and_fork_tip_roots(&database)
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        roots_after_each_best_block(BlockNumber::from(deepest_fork_block_offset)),
        roots_after_each_best_block(BlockNumber::MAX)
    );
}
