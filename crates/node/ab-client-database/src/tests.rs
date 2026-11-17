use crate::storage_backend::{
    ClientDatabaseStorageBackend, OwnedPages, SharedPages, StorageBackendError,
};
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::test_utils::{
    Database, DatabaseOptions, IntermediateShardBlockSegments, MemoryStorageBackend,
    PAGE_GROUP_SIZE, beacon_chain_block, beacon_chain_block_with_shard_segments,
    block_confirmation_depth, database_options, genesis_block_builder, highest_persisted_number,
    open_database, open_database_with_options, persist_beacon_chain_block, persist_block,
    persist_chain, reopen, segment_header, super_segment_header,
};
use crate::{
    ClientDatabase, ClientDatabaseBlock, ClientDatabaseError, ClientDatabaseFormatError,
    ClientDatabaseFormatOptions,
};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_client_api::{
    BeaconChainInfo, BeaconChainInfoWrite, BlockDetails, ChainInfo, ChainInfoWrite,
    ContractSlotState, PersistBlockError, PersistSegmentHeadersError,
    PersistSuperSegmentHeadersError, ReadBlockError, ShardSegmentRoot,
};
use ab_core_primitives::address::Address;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{LocalSegmentIndex, SegmentRoot, SuperSegmentIndex};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::trees::BlockMerkleMountainRange;
use futures::channel::oneshot;
use futures::executor::block_on;
use std::num::{NonZeroU32, NonZeroUsize};
use std::{assert_matches, io, iter};
use triomphe::Arc;

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
/// fork offset `0`, that block offsets from `block_confirmation_depth - 1` have no forks, that
/// exactly the ancestors at block offsets from `block_confirmation_depth` are confirmed and that
/// all of them can be read
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
            #[expect(
                clippy::rest_pattern_accessible_field,
                reason = "Do not need other fields"
            )]
            let confirmed = matches!(
                canonical_block,
                ClientDatabaseBlock::PersistedConfirmed { .. }
            );
            assert_eq!(
                confirmed,
                block_offset >= block_confirmation_depth,
                "Confirmation at block offset {block_offset}"
            );
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

/// Check that blocks from genesis on have `shard_segment_roots`
fn assert_shard_segment_roots(database: &Database, shard_segment_roots: &[&[ShardSegmentRoot]]) {
    for (number, shard_segment_roots) in (0..).zip(shard_segment_roots) {
        assert_eq!(
            *database
                .shard_segment_roots(BlockNumber::from(number))
                .unwrap(),
            **shard_segment_roots,
            "{number}"
        );
    }
}

#[test]
fn shard_segment_roots_survive_persisting_and_reopen() {
    // genesis - c1 (segments of intermediate and leaf shards) - c2 (no segments) - c3 (segments of
    // an intermediate shard) - c4 .. c6 (`c3` is written at soft confirmation depth, `c1` is
    // confirmed, `c4 .. c6` are not persisted)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let intermediate_shard_1 = ShardIndex::new(1).unwrap();
    let intermediate_shard_2 = ShardIndex::new(2).unwrap();
    // Leaf shards are children of intermediate shards with the same least significant bits
    let leaf_shard_1_1 = ShardIndex::new(1 | (1 << 10)).unwrap();
    let leaf_shard_1_2 = ShardIndex::new(1 | (2 << 10)).unwrap();
    let leaf_shard_2_1 = ShardIndex::new(2 | (1 << 10)).unwrap();
    let segment = |local_segment_index, root| {
        (
            LocalSegmentIndex::from(local_segment_index),
            SegmentRoot::from([root; _]),
        )
    };
    let shard_segment_root = |shard_index, local_segment_index, root| ShardSegmentRoot {
        shard_index,
        segment_index: LocalSegmentIndex::from(local_segment_index),
        segment_root: SegmentRoot::from([root; _]),
    };

    let mut chain = vec![database.best_root()];
    chain.push(
        persist_beacon_chain_block(
            &database,
            beacon_chain_block_with_shard_segments(
                1,
                chain[0],
                &[
                    IntermediateShardBlockSegments {
                        shard_index: intermediate_shard_1,
                        own_segments: &[segment(0, 1)],
                        leaf_shard_blocks: &[
                            (leaf_shard_1_1, &[]),
                            (leaf_shard_1_1, &[segment(0, 2), segment(1, 3)]),
                            (leaf_shard_1_2, &[segment(5, 4)]),
                        ],
                    },
                    IntermediateShardBlockSegments {
                        shard_index: intermediate_shard_1,
                        own_segments: &[segment(1, 5)],
                        leaf_shard_blocks: &[],
                    },
                    IntermediateShardBlockSegments {
                        shard_index: intermediate_shard_2,
                        own_segments: &[],
                        leaf_shard_blocks: &[(leaf_shard_2_1, &[segment(0, 6)])],
                    },
                ],
            ),
        )
        .unwrap(),
    );
    chain.push(persist_block(&database, 2, chain[1], 0).unwrap());
    chain.push(
        persist_beacon_chain_block(
            &database,
            beacon_chain_block_with_shard_segments(
                3,
                chain[2],
                &[IntermediateShardBlockSegments {
                    shard_index: intermediate_shard_1,
                    own_segments: &[segment(2, 7)],
                    leaf_shard_blocks: &[],
                }],
            ),
        )
        .unwrap(),
    );
    // Own segments of each intermediate shard block are followed by segments of its leaf shards
    let shard_segment_roots = [
        &[][..],
        &[
            shard_segment_root(intermediate_shard_1, 0, 1),
            shard_segment_root(leaf_shard_1_1, 0, 2),
            shard_segment_root(leaf_shard_1_1, 1, 3),
            shard_segment_root(leaf_shard_1_2, 5, 4),
            shard_segment_root(intermediate_shard_1, 1, 5),
            shard_segment_root(leaf_shard_2_1, 0, 6),
        ],
        &[],
        &[shard_segment_root(intermediate_shard_1, 2, 7)],
    ];
    assert_shard_segment_roots(&database, &shard_segment_roots);

    chain.extend(persist_chain(&database, chain[3], 4..=6, 0));
    {
        let state = database.inner.state.read_blocking();
        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        let persisted = matches!(
            state.data.blocks[3].as_slice(),
            [ClientDatabaseBlock::Persisted { .. }]
        );
        assert!(persisted);
        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        let confirmed = matches!(
            state.data.blocks[5].as_slice(),
            [ClientDatabaseBlock::PersistedConfirmed { .. }]
        );
        assert!(confirmed);
    }
    assert_shard_segment_roots(&database, &shard_segment_roots);

    // Bodies are read back up to `c3`
    let database = reopen(database, &storage_backend);
    assert_eq!(database.best_root(), chain[3]);
    assert_shard_segment_roots(&database, &shard_segment_roots);
}

#[test]
fn rejected_segment_headers_are_not_added() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);

    assert_matches!(
        block_on(database.persist_segment_headers(vec![segment_header(1)])),
        Err(PersistSegmentHeadersError::FirstSegmentIndexZero { local_segment_index })
            if local_segment_index == LocalSegmentIndex::ONE
    );
    assert_eq!(database.last_segment_header(), None);

    block_on(database.persist_segment_headers(vec![segment_header(0), segment_header(1)])).unwrap();
    // Segment header `2` follows the stored ones, but the batch is rejected as a whole
    assert_matches!(
        block_on(database.persist_segment_headers(vec![segment_header(2), segment_header(4)])),
        Err(PersistSegmentHeadersError::MustFollowLastSegmentIndex {
            local_segment_index,
            last_local_segment_index,
        }) if local_segment_index == LocalSegmentIndex::from(4)
            && last_local_segment_index == LocalSegmentIndex::from(2)
    );
    assert_eq!(database.last_segment_header(), Some(segment_header(1)));
    assert_eq!(
        database.get_segment_header(LocalSegmentIndex::from(2)),
        None
    );

    block_on(database.persist_segment_headers(vec![segment_header(2)])).unwrap();

    let database = reopen(database, &storage_backend);

    assert_eq!(database.last_segment_header(), Some(segment_header(2)));
}

#[test]
fn rejected_super_segment_headers_are_not_added() {
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);

    assert_matches!(
        block_on(database.persist_super_segment_header(super_segment_header(1))),
        Err(PersistSuperSegmentHeadersError::FirstSegmentIndexZero { super_segment_index })
            if super_segment_index == SuperSegmentIndex::ONE
    );
    assert_eq!(database.last_super_segment_header(), None);

    block_on(
        database
            .persist_super_segment_headers(vec![super_segment_header(0), super_segment_header(1)]),
    )
    .unwrap();
    // Super segment header `2` follows the stored ones, but the batch is rejected as a whole
    assert_matches!(
        block_on(
            database.persist_super_segment_headers(vec![
                super_segment_header(2),
                super_segment_header(4),
            ])
        ),
        Err(PersistSuperSegmentHeadersError::MustFollowLastSegmentIndex {
            super_segment_index,
            last_super_segment_index,
        }) if super_segment_index == SuperSegmentIndex::from(4)
            && last_super_segment_index == SuperSegmentIndex::from(2)
    );
    assert_eq!(
        database.last_super_segment_header(),
        Some(super_segment_header(1))
    );
    assert_eq!(
        database.get_super_segment_header(SuperSegmentIndex::from(2)),
        None
    );

    assert!(block_on(database.persist_super_segment_header(super_segment_header(2))).unwrap());

    let database = reopen(database, &storage_backend);

    assert_eq!(
        database.last_super_segment_header(),
        Some(super_segment_header(2))
    );
}

#[test]
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
    assert_matches!(
        block_on(database.block(&a3)),
        Err(ReadBlockError::UnknownBlockRoot)
    );
    for block_root in [b3, d3] {
        assert!(database.header(&block_root).is_some());
        block_on(database.block(&block_root)).unwrap();
    }
    assert_eq!(database.best_root(), chain[3]);
}

#[test]
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
        assert_matches!(
            persist_block(&database, number, unknown_root, 0),
            Err(PersistBlockError::MissingParent)
        );
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

    assert_matches!(
        persist_block(&database, 4, a3, 1),
        Err(PersistBlockError::MissingParent)
    );
    assert_best_chain(&database, &chain);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn known_block_is_rejected() {
    // genesis - c1 .. c10 (best block), then `c(10 - B)` is persisted again
    //
    // `B` goes up to `block_confirmation_depth - 2`, the deepest block offset where fork blocks are
    // accepted. A copy at a block offset up to `soft_confirmation_depth` would be written to disk,
    // a deeper copy would only be kept in memory.
    let block_confirmation_depth = 8;
    for block_offset in 0..=block_confirmation_depth - 2 {
        let storage_backend = MemoryStorageBackend::new();
        let database = open_database_with_options(DatabaseOptions {
            block_confirmation_depth: BlockNumber::from(block_confirmation_depth),
            ..database_options(&storage_backend)
        })
        .unwrap();
        let mut chain = vec![database.best_root()];
        chain.extend(persist_chain(&database, chain[0], 1..=10, 0));

        let number = 10 - block_offset;
        let parent_root = chain[usize::try_from(number - 1).unwrap()];
        let roots_before = block_and_fork_tip_roots(&database);
        assert_matches!(
            persist_block(&database, number, parent_root, 0),
            Err(PersistBlockError::AlreadyKnown),
            "{block_offset}"
        );
        assert_eq!(block_and_fork_tip_roots(&database), roots_before);
        assert!(
            database
                .header(&chain[usize::try_from(number).unwrap()])
                .is_some()
        );
        assert_best_chain(&database, &chain);
        assert_consistent(&database, 10);

        // Past the block offset where the copy would have been pruned together with the original
        // block
        extend_and_reopen(database, &storage_backend, block_confirmation_depth, 1);
    }
}

#[test]
fn known_genesis_block_is_rejected() {
    // The genesis block on a fresh database takes the same path as the first block synced on top
    // of it, which replaces the whole state
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let genesis_root = database.best_root();
    let roots_before = block_and_fork_tip_roots(&database);

    assert_matches!(
        persist_block(&database, 0, BlockRoot::default(), 0),
        Err(PersistBlockError::AlreadyKnown)
    );
    assert_eq!(block_and_fork_tip_roots(&database), roots_before);
    assert!(database.header(&genesis_root).is_some());
    assert_best_chain(&database, &[genesis_root]);
    persist_block(&database, 1, genesis_root, 0).unwrap();
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

    assert_matches!(
        persist_block(&database, 4, chain[3], 1),
        Err(PersistBlockError::OutsideAcceptableRange)
    );
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
    assert_matches!(
        persist_block(&database, 5, fork_chain[2], 1),
        Err(PersistBlockError::MissingParent)
    );
    assert!(database.header(&b5).is_some());
    persist_block(&database, 6, b5, 3).unwrap();
    assert_best_chain(&database, &chain);
}

#[test]
fn block_details_are_dropped_from_memory_at_block_confirmation_depth() {
    // genesis - c1 .. c4 -+- c5 .. c(3 + block_confirmation_depth) (best block)
    //                     \- a5 (block offset `block_confirmation_depth - 2`, the deepest accepted
    //                         fork block, block details of its parent `c4` are still in memory)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let block_confirmation_depth = block_confirmation_depth(&database);
    let best_number = 3 + block_confirmation_depth;
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=best_number, 0));

    {
        let state = database.inner.state.read_blocking();
        let block_offset = usize::try_from(block_confirmation_depth).unwrap();
        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        let confirmed = matches!(
            state.data.blocks[block_offset].as_slice(),
            [ClientDatabaseBlock::PersistedConfirmed { .. }]
        );
        assert!(confirmed);
    }
    for (number, block_root) in (0..).zip(&chain) {
        let block_offset = best_number - number;
        assert_eq!(
            database.header_with_details(block_root).is_some(),
            block_offset < block_confirmation_depth,
            "{block_offset}"
        );
        assert!(database.header(block_root).is_some(), "{block_offset}");
    }

    // The deepest fork block that is accepted, block import reads block details of its parent
    let fork_block_offset =
        u64::try_from(database.inner.options.fork_pruning_block_offset()).unwrap() - 1;
    let fork_number = best_number - fork_block_offset;
    let parent_root = chain[usize::try_from(fork_number - 1).unwrap()];
    assert!(database.header_with_details(&parent_root).is_some());
    persist_block(&database, fork_number, parent_root, 1).unwrap();
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
    assert_matches!(
        persist_block(&database, 6, a5, 1),
        Err(PersistBlockError::MissingParent)
    );
    assert_matches!(
        persist_block(
            &database,
            persisted_best_number + 1 - block_confirmation_depth,
            chain[4],
            2
        ),
        Err(PersistBlockError::OutsideAcceptableRange)
    );

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
        assert_matches!(
            open_database_with_options(options_with_depth(block_confirmation_depth)),
            Err(ClientDatabaseError::InvalidBlockConfirmationDepth),
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

    assert_matches!(
        open_database_with_options(DatabaseOptions {
            soft_confirmation_depth: block_confirmation_depth,
            ..options.clone()
        }),
        Err(ClientDatabaseError::InvalidSoftConfirmationDepth)
    );
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

/// Whether `block_root` is one of the fork tips
fn is_fork_tip(database: &Database, block_root: &BlockRoot) -> bool {
    let state = database.inner.state.read_blocking();
    state
        .data
        .fork_tips
        .iter()
        .any(|fork_tip| fork_tip.root == *block_root)
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn fork_beyond_max_fork_tip_distance_is_pruned_before_it_is_written() {
    // genesis - c1 .. c3 -+- c4 .. c7 (`a5` is at distance 2) .. c9 (best block)
    //                     \- a4 - a5 (pruned with `a4` at `c7`, before `a4` is persisted)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database_with_options(DatabaseOptions {
        max_fork_tip_distance: BlockNumber::ONE,
        ..database_options(&storage_backend)
    })
    .unwrap();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=5, 0));
    let a4 = persist_block(&database, 4, chain[3], 1).unwrap();
    let a5 = persist_block(&database, 5, a4, 1).unwrap();

    chain.push(persist_block(&database, 6, chain[5], 0).unwrap());
    assert!(is_fork_tip(&database, &a5));

    chain.push(persist_block(&database, 7, chain[6], 0).unwrap());
    for block_root in [a4, a5] {
        assert!(database.header(&block_root).is_none());
    }
    assert_matches!(
        persist_block(&database, 6, a5, 1),
        Err(PersistBlockError::MissingParent)
    );

    chain.extend(persist_chain(&database, chain[7], 8..=9, 0));
    let database = reopen(database, &storage_backend);
    assert_consistent(&database, highest_persisted_number(&database, 9));
    for block_root in [a4, a5] {
        assert!(database.header(&block_root).is_none());
    }
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn fork_in_use_is_kept_beyond_max_fork_tip_distance() {
    // genesis - c1 .. c3 -+- c4 .. c8 (best block)
    //                     +- a4 (in use until `c6`, pruned at `c7`)
    //                     \- b4 (in use, pruned at `c8` at block offset
    //                         `block_confirmation_depth - 1` nevertheless)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database_with_options(DatabaseOptions {
        max_fork_tip_distance: BlockNumber::ONE,
        ..database_options(&storage_backend)
    })
    .unwrap();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=4, 0));
    let [a4, b4] = [1, 2].map(|salt| persist_block(&database, 4, chain[3], salt).unwrap());
    let [a4_header, b4_header] = [a4, b4].map(|block_root| database.header(&block_root).unwrap());

    chain.extend(persist_chain(&database, chain[4], 5..=6, 0));
    for block_root in [a4, b4] {
        assert!(database.header(&block_root).is_some());
        assert!(is_fork_tip(&database, &block_root));
    }

    drop(a4_header);
    chain.push(persist_block(&database, 7, chain[6], 0).unwrap());
    assert!(database.header(&a4).is_none());
    assert!(database.header(&b4).is_some());

    chain.push(persist_block(&database, 8, chain[7], 0).unwrap());
    assert!(database.header(&b4).is_none());
    assert_eq!(*b4_header.header().root(), b4);
    assert_best_chain(&database, &chain);
}

#[test]
#[cfg_attr(miri, ignore = "Fork logic is safe code, too slow under Miri")]
fn persisted_fork_beyond_max_fork_tip_distance_is_kept_until_fork_pruning_block_offset() {
    // genesis - c1 - c2 -+- c3 .. c7 (best block)
    //                    \- a3 (persisted at `c4`, no longer a fork tip at `c5`, pruned at `c7`)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database_with_options(DatabaseOptions {
        soft_confirmation_depth: BlockNumber::ONE,
        max_fork_tip_distance: BlockNumber::ONE,
        ..database_options(&storage_backend)
    })
    .unwrap();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=3, 0));
    let a3 = persist_block(&database, 3, chain[2], 1).unwrap();

    chain.extend(persist_chain(&database, chain[3], 4..=5, 0));
    assert!(!is_fork_tip(&database, &a3));
    let block = block_on(database.block(&a3)).unwrap();
    assert_eq!(*block.header.header().root(), a3);

    chain.push(persist_block(&database, 6, chain[5], 0).unwrap());
    assert!(database.header(&a3).is_some());

    chain.push(persist_block(&database, 7, chain[6], 0).unwrap());
    assert!(database.header(&a3).is_none());
    assert_best_chain(&database, &chain);
}

#[test]
fn format_without_force_only_formats_unformatted_storage() {
    let storage_backend = MemoryStorageBackend::new();
    let options = ClientDatabaseFormatOptions {
        page_group_size: PAGE_GROUP_SIZE,
        force: false,
    };

    assert_matches!(
        block_on(Database::format(&storage_backend, options)),
        Err(ClientDatabaseFormatError::AlreadyFormatted)
    );

    // Erase the page group header, as if the storage was never formatted
    block_on(storage_backend.write(OwnedPages::zeroed(OwnedPages::ALIGN).into_shared(), 0))
        .unwrap()
        .0
        .unwrap();
    block_on(Database::format(&storage_backend, options)).unwrap();
    open_database(&storage_backend);

    assert_matches!(
        block_on(Database::format(&storage_backend, options)),
        Err(ClientDatabaseFormatError::AlreadyFormatted)
    );
}

/// Storage backend with the max number of pages, which is never read from or written to
#[derive(Debug)]
struct HugeStorageBackend;

impl ClientDatabaseStorageBackend for HugeStorageBackend {
    fn num_pages(&self) -> u32 {
        u32::MAX
    }

    fn read(
        &self,
        _buffer: OwnedPages,
        _num_pages: u32,
        _offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, OwnedPages)> {
        unreachable!("Not read from")
    }

    fn write(
        &self,
        _buffer: SharedPages,
        _offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, SharedPages)> {
        unreachable!("Not written to")
    }
}

#[test]
fn format_rejects_invalid_page_group_size() {
    let storage_backend = MemoryStorageBackend::new();
    let num_pages = storage_backend.num_pages();

    assert_matches!(
        block_on(Database::format(
            &storage_backend,
            ClientDatabaseFormatOptions {
                page_group_size: NonZeroU32::MIN,
                force: true,
            },
        )),
        Err(ClientDatabaseFormatError::PageGroupSizeTooSmall { page_group_size: 1 })
    );

    let page_group_size = num_pages + 1;
    let result = block_on(Database::format(
        &storage_backend,
        ClientDatabaseFormatOptions {
            page_group_size: NonZeroU32::new(page_group_size).unwrap(),
            force: true,
        },
    ));
    let Err(ClientDatabaseFormatError::PageGroupSizeTooLarge {
        page_group_size: error_page_group_size,
        num_pages: error_num_pages,
    }) = result
    else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(
        (error_page_group_size, error_num_pages),
        (page_group_size, num_pages)
    );

    // Storage backend is large enough, but the page group doesn't fit into a buffer for reading it
    let result = block_on(ClientDatabase::<OwnedBeaconChainBlock, _>::format(
        &HugeStorageBackend,
        ClientDatabaseFormatOptions {
            page_group_size: NonZeroU32::MAX,
            force: true,
        },
    ));
    assert_matches!(
        result,
        Err(
            ClientDatabaseFormatError::PageGroupSizeExceedsBufferCapacity {
                page_group_size: u32::MAX
            }
        )
    );

    // The previously formatted database is left intact
    open_database(&storage_backend);
}

#[test]
fn block_with_too_large_system_contract_states_is_not_written() {
    // genesis - c1 (system contract states over `u32::MAX` bytes) - c2 .. c4 (`c1` is written
    // when `c4` is imported)
    let storage_backend = MemoryStorageBackend::new();
    let database = open_database(&storage_backend);
    let c1 = beacon_chain_block(1, database.best_root(), 0);
    let c1_root = *c1.header.header().root();
    let block_details = BlockDetails {
        mmr_with_block: Arc::new(BlockMerkleMountainRange::new()),
        // All states share the same contents to keep memory usage low
        system_contract_states: iter::repeat_n(
            ContractSlotState {
                owner: Address::NULL,
                contract: Address::SYSTEM_STATE,
                contents: SharedAlignedBuffer::from_bytes(&vec![0; 1024 * 1024]),
            },
            4096,
        )
        .collect(),
    };
    block_on(database.persist_block(c1, block_details)).unwrap();
    let chain = persist_chain(&database, c1_root, 2..=3, 0);
    assert_eq!(highest_persisted_number(&database, 4), 1);

    let result = persist_block(&database, 4, chain[1], 0);
    let Err(PersistBlockError::StorageItemWriteError { error }) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(error.kind(), io::ErrorKind::QuotaExceeded);
    assert_matches!(
        *error.into_inner().unwrap().downcast().unwrap(),
        StorageItemError::StorageItemTooLarge
    );
}

/// Options for a database on `storage_backend` that buffers `write_buffer_size` writes
fn buffered_database_options(
    storage_backend: &MemoryStorageBackend,
    write_buffer_size: usize,
) -> DatabaseOptions {
    DatabaseOptions {
        write_buffer_size,
        ..database_options(storage_backend)
    }
}

/// Check that `block` is the block built by [`beacon_chain_block()`] at `number` of `chain`, which
/// starts with genesis and was built with zero salt
fn assert_chain_block(block: &OwnedBeaconChainBlock, chain: &[BlockRoot], number: u64) {
    let parent_root = number
        .checked_sub(1)
        .map_or_else(BlockRoot::default, |parent_number| {
            chain[usize::try_from(parent_number).unwrap()]
        });
    let expected = beacon_chain_block(number, parent_root, 0);
    assert_eq!(
        block.header.buffer().as_slice(),
        expected.header.buffer().as_slice(),
        "{number}"
    );
    assert_eq!(
        block.body.buffer().as_slice(),
        expected.body.buffer().as_slice(),
        "{number}"
    );
}

/// Check that all blocks of `chain`, which starts with genesis and was built with zero salt, can be
/// read
fn assert_chain_blocks(database: &Database, chain: &[BlockRoot]) {
    for (number, block_root) in (0..).zip(chain) {
        let block = block_on(database.block(block_root)).unwrap();
        assert_chain_block(&block, chain, number);
    }
}

#[test]
#[cfg_attr(miri, ignore = "Reopening is safe code, too slow under Miri")]
fn persisted_blocks_are_read_while_their_writes_are_in_flight() {
    // genesis - c1 .. c8, blocks up to `c5` are persisted and not kept in memory
    let storage_backend = MemoryStorageBackend::new();
    let database =
        open_database_with_options(buffered_database_options(&storage_backend, 16)).unwrap();
    // Genesis block is written once `c3` is persisted and, as the first temporary storage item,
    // starts a temporary page group, so it is written after the page group header
    storage_backend.hold_writes();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=8, 0));
    assert_chain_blocks(&database, &chain);

    storage_backend.release_writes();
    assert_chain_blocks(&database, &chain);

    let database = reopen(database, &storage_backend);
    let persisted_best_number = highest_persisted_number(&database, 8);
    assert_consistent(&database, persisted_best_number);
    assert_chain_blocks(
        &database,
        &chain[..=usize::try_from(persisted_best_number).unwrap()],
    );
}

#[test]
fn persisted_blocks_are_read_after_their_writes_failed() {
    // genesis - c1 .. c9, writes of blocks up to `c5` fail, the failure is observed when `c6` is
    // written, since all six entries of the write buffer are occupied by failed writes
    let storage_backend = MemoryStorageBackend::new();
    let database =
        open_database_with_options(buffered_database_options(&storage_backend, 6)).unwrap();
    storage_backend.hold_writes();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=8, 0));
    storage_backend.fail_held_writes();

    let result = persist_block(&database, 9, chain[8], 0);
    let Err(PersistBlockError::StorageItemWriteError { error }) = result else {
        panic!("Unexpected result: {result:?}");
    };
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_chain_blocks(&database, &chain);
}

#[test]
fn blocks_read_from_in_flight_writes_outlive_reuse_of_write_buffer() {
    // genesis - c1 .. c6, `genesis` and `c1` are read while their writes are in flight and remain
    // intact after writes of `c2` and `c3` reuse the write buffer
    let storage_backend = MemoryStorageBackend::new();
    let database =
        open_database_with_options(buffered_database_options(&storage_backend, 2)).unwrap();
    storage_backend.hold_writes();
    let mut chain = vec![database.best_root()];
    chain.extend(persist_chain(&database, chain[0], 1..=4, 0));
    let blocks = chain
        .iter()
        .take(2)
        .map(|block_root| block_on(database.block(block_root)).unwrap())
        .collect::<Vec<_>>();

    storage_backend.release_writes();
    chain.extend(persist_chain(&database, chain[4], 5..=6, 0));
    for (number, block) in (0..).zip(&blocks) {
        assert_chain_block(block, &chain, number);
    }
    assert_chain_blocks(&database, &chain);
}
