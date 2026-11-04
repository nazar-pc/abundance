use crate::task::SegmentArchiverTaskError;
use crate::test_utils::{
    ArchiverTask, BeaconChainBlockBuilder, LeafShardBlockBuilder, MockChain, run,
};
use ab_client_api::{ChainInfo, ChainInfoWrite, ReadBlockError};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::{
    ArchivedBlockProgress, LastArchivedBlock, LocalSegmentIndex, RecordedHistorySegment,
    SegmentHeader, SegmentRoot,
};
use std::assert_matches;

/// Blocks with this body size fill a segment in less than three blocks
const LARGE_BODY_SIZE: usize = RecordedHistorySegment::SIZE * 3 / 8;

/// Local segment indices and last archived block numbers of `segment_headers`, checking that each
/// segment header follows the previous one
fn segments(segment_headers: &[SegmentHeader]) -> Vec<(u64, u64)> {
    for (previous_segment_header, segment_header) in
        segment_headers.iter().zip(segment_headers.iter().skip(1))
    {
        assert_eq!(
            segment_header.prev_segment_header_hash,
            previous_segment_header.hash()
        );
    }

    segment_headers
        .iter()
        .map(|segment_header| {
            (
                u64::from(segment_header.index.as_inner()),
                u64::from(segment_header.last_archived_block.number()),
            )
        })
        .collect()
}

/// Segment headers that Snap sync stores before it inserts block `last_archived_block_number`
/// directly: segment 0 with the genesis block and segment 1 that ends with that block
fn synced_segment_headers(last_archived_block_number: BlockNumber) -> Vec<SegmentHeader> {
    let segment_header_0 = SegmentHeader {
        index: LocalSegmentIndex::ZERO.into(),
        root: SegmentRoot::from([0; _]),
        prev_segment_header_hash: Blake3Hash::default(),
        last_archived_block: LastArchivedBlock {
            number: BlockNumber::ZERO.into(),
            archived_progress: ArchivedBlockProgress::new_complete(),
        },
    };
    let segment_header_1 = SegmentHeader {
        index: LocalSegmentIndex::ONE.into(),
        root: SegmentRoot::from([1; _]),
        prev_segment_header_hash: segment_header_0.hash(),
        last_archived_block: LastArchivedBlock {
            number: last_archived_block_number.into(),
            archived_progress: ArchivedBlockProgress::new_complete(),
        },
    };

    vec![segment_header_0, segment_header_1]
}

#[test]
#[cfg_attr(miri, ignore = "Archives segments of 128 MiB, too slow under Miri")]
fn archives_blocks_into_segments() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let chain = MockChain::new(genesis_block.clone());

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        // The genesis block is padded to fill the first segment, which is archived right away
        // without a notification
        assert_eq!(segments(&chain.segment_headers()), [(0, 0)]);

        // The next segments end in the middle of blocks 3 and 6, and each block is archived when
        // the block `block_confirmation_depth` blocks later is about to be imported:
        //
        // block being imported  c1  c2  c3  c4  c5  c6  c7  c8  c9
        // block to archive       -   -   -  c1  c2  c3  c4  c5  c6
        // segment archived                           1           2
        let mut parent = genesis_block;
        for _ in 1..=9 {
            let block = block_builder.child(&parent);
            task.import(block.clone()).await;
            parent = block;
        }

        assert_eq!(
            task.archived_segment_indices(),
            [LocalSegmentIndex::from(1), LocalSegmentIndex::from(2)]
        );
        assert_eq!(segments(&chain.segment_headers()), [(0, 0), (1, 3), (2, 6)]);

        task.stop().await.unwrap();
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives a segment of 128 MiB, too slow under Miri")]
fn continues_after_block_inserted_by_snap_sync() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        let chain = MockChain::new(block_builder.genesis());

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        // Snap sync stores all known segment headers first, the last one ends with `c20`, then
        // inserts `c20` directly, without its ancestors:
        //
        // genesis   (missing)   c20 - c21 - .. - c26
        let segment_header_0 = chain.last_segment_header().unwrap();
        chain
            .persist_segment_headers(vec![SegmentHeader {
                index: LocalSegmentIndex::ONE.into(),
                root: SegmentRoot::from([1; _]),
                prev_segment_header_hash: segment_header_0.hash(),
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::from(20).into(),
                    archived_progress: ArchivedBlockProgress::new_complete(),
                },
            }])
            .await
            .unwrap();
        let c20 = block_builder.block(BlockNumber::from(20), BlockRoot::default());
        chain.add_block(c20.clone());

        // Blocks below `c20` are skipped, `c20` is already archived in segment 1, which is found
        // by re-initialization when `c23` is about to be imported, and archiving continues after
        // it:
        //
        // block being imported  c21  c22  c23  c24  c25  c26
        // block to archive       18   19   20   21   22   23
        //                        \-----/   |    \----------/
        //                        skipped   |      archived
        //                                  already archived, found by re-initialization
        let mut parent = c20;
        for _ in 21..=26 {
            let block = block_builder.child(&parent);
            task.import(block.clone()).await;
            parent = block;
        }

        task.stop().await.unwrap();
        // The genesis block was archived on start, `c20` is read to resume archiving after it
        assert_eq!(
            chain.read_block_numbers(),
            [0, 20, 21, 22, 23].map(BlockNumber::from)
        );
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives a segment of 128 MiB, too slow under Miri")]
fn gap_after_reinitialization_is_an_error() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        let genesis_block = block_builder.genesis();
        let chain = MockChain::new(genesis_block.clone());

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        let mut parent = genesis_block;
        for _ in 1..=10 {
            let block = block_builder.child(&parent);
            task.import(block.clone()).await;
            parent = block;
        }

        // `c12` is about to be imported while its parent `c11` is not in the chain, which block
        // import never does:
        //
        // genesis - c1 - .. - c10   (c11 missing)   c12
        let c11 = block_builder.child(&parent);
        let c12 = block_builder.child(&c11);
        task.import(c12).await;

        assert_matches!(
            task.stop().await,
            Err(SegmentArchiverTaskError::BlockGap {
                best_archived_block_number,
                block_number_to_archive,
                importing_block_number,
            }) if best_archived_block_number == BlockNumber::from(7)
                && block_number_to_archive == BlockNumber::from(9)
                && importing_block_number == BlockNumber::from(12)
        );
    });
}

#[test]
fn missing_block_is_an_error() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        // `c20` was inserted directly, but the only known segment header is that of segment 0,
        // whose last archived block is the genesis block. The chain doesn't have it, so
        // re-initialization can't resume from segment 0 and starts from the genesis block.
        let c20 = block_builder.block(BlockNumber::from(20), BlockRoot::default());
        let chain = MockChain::new(c20);
        chain
            .persist_segment_headers(vec![SegmentHeader {
                index: LocalSegmentIndex::ZERO.into(),
                root: SegmentRoot::from([0; _]),
                prev_segment_header_hash: Blake3Hash::default(),
                last_archived_block: LastArchivedBlock {
                    number: BlockNumber::ZERO.into(),
                    archived_progress: ArchivedBlockProgress::new_complete(),
                },
            }])
            .await
            .unwrap();

        // Re-initialization happens in the task since a segment header is known already
        let task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        assert_matches!(
            task.stop().await,
            Err(SegmentArchiverTaskError::BlockMissing { block_number })
                if block_number == BlockNumber::ZERO
        );
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives a segment of 128 MiB, too slow under Miri")]
fn unreadable_block_is_an_error() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        let genesis_block = block_builder.genesis();
        let chain = MockChain::new(genesis_block.clone());

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);
        let c6 = block_builder.child(&c5);
        let c3_root = *c3.header.header().root();
        for block in [c1, c2, c3, c4, c5] {
            task.import(block).await;
        }

        // `c3` is archived when `c6` is about to be imported, but can't be read
        chain.make_block_unreadable(c3_root);
        task.import(c6).await;

        assert_matches!(
            task.stop().await,
            Err(SegmentArchiverTaskError::ReadBlock {
                block_number,
                error: ReadBlockError::FailedToDecode,
            }) if block_number == BlockNumber::from(3)
        );
    });
}

#[test]
fn unreadable_block_on_initialization_is_an_error() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        // Snap sync stored segment headers and inserted `c20`, then `c21` to `c25` were imported
        let c20 = block_builder.block(BlockNumber::from(20), BlockRoot::default());
        let c21 = block_builder.child(&c20);
        let c22 = block_builder.child(&c21);
        let c23 = block_builder.child(&c22);
        let c24 = block_builder.child(&c23);
        let c25 = block_builder.child(&c24);
        let chain = MockChain::new(c20);
        chain
            .persist_segment_headers(synced_segment_headers(BlockNumber::from(20)))
            .await
            .unwrap();
        chain.make_block_unreadable(*c22.header.header().root());
        for block in [c21, c22, c23, c24, c25] {
            chain.add_block(block);
        }

        // Re-initialization happens in the task since segment headers are known already. It
        // resumes after `c20` and archives blocks up to `c22`, which is `block_confirmation_depth`
        // blocks below `c25`, but `c22` can't be read.
        let task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        assert_matches!(
            task.stop().await,
            Err(SegmentArchiverTaskError::ReadBlock {
                block_number,
                error: ReadBlockError::FailedToDecode,
            }) if block_number == BlockNumber::from(22)
        );
    });
}

#[test]
fn missing_block_to_archive_is_an_error() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(0);
        let c20 = block_builder.block(BlockNumber::from(20), BlockRoot::default());
        let chain = MockChain::new(c20.clone());
        chain
            .persist_segment_headers(synced_segment_headers(BlockNumber::from(20)))
            .await
            .unwrap();

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        // Re-initialization resumes after `c20`, and `c21` confirms `c18`, which is skipped
        let c21 = block_builder.child(&c20);
        task.import(c21).await;

        // Snap sync inserts `d23` directly without storing a segment header for it, so `c21`,
        // which is archived when `d24` is about to be imported, is not in the best chain anymore:
        //
        // (missing)   c20 - c21
        //
        // (missing)               d23 - d24
        let d23 = block_builder.block(BlockNumber::from(23), BlockRoot::default());
        let d24 = block_builder.child(&d23);
        chain.add_block(d23);
        task.import(d24).await;

        assert_matches!(
            task.stop().await,
            Err(SegmentArchiverTaskError::BlockMissing { block_number })
                if block_number == BlockNumber::from(21)
        );
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives a segment of 128 MiB, too slow under Miri")]
fn archives_leaf_shard_from_genesis() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = LeafShardBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);
        let chain = MockChain::new(genesis_block);

        // Unlike that of the beacon chain, the genesis block of a leaf shard is not padded to fill
        // a segment, so archiving it on start doesn't produce one, and neither does archiving
        // `c1`. The first segment ends in the middle of `c2`, which is archived when `c5` is about
        // to be imported, after a restart that archives blocks up to `c1` again:
        //
        // block being imported  c1  c2  c3  c4  (restart)  c5
        // block to archive       -   -   -  c1             c2
        // segment archived                                  0
        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [c1, c2, c3, c4] {
            task.import(block).await;
        }
        task.stop().await.unwrap();
        assert_eq!(segments(&chain.segment_headers()), []);

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        task.import(c5).await;
        assert_eq!(task.archived_segment_indices(), [LocalSegmentIndex::ZERO]);
        assert_eq!(segments(&chain.segment_headers()), [(0, 2)]);
        task.stop().await.unwrap();
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives segments of 128 MiB, too slow under Miri")]
fn restart_resumes_from_first_segment() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);
        let c6 = block_builder.child(&c5);

        // Segment 1 ends in `c3`, which is archived when `c6` is about to be imported
        let reference_chain = MockChain::new(genesis_block.clone());
        let mut task = ArchiverTask::start(&reference_chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [&c1, &c2, &c3, &c4, &c5, &c6] {
            task.import(block.clone()).await;
        }
        task.stop().await.unwrap();
        assert_eq!(
            segments(&reference_chain.segment_headers()),
            [(0, 0), (1, 3)]
        );

        // Only segment 0 is stored when the node restarts. Re-initialization resumes from it
        // instead of creating it again, and archives `c1` again:
        //
        // block being imported  c1  c2  c3  c4  (restart)  c5  c6
        // block to archive       -   -   -  c1         c1  c2  c3
        // segment archived                                      1
        let chain = MockChain::new(genesis_block);
        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [c1, c2, c3, c4] {
            task.import(block).await;
        }
        task.stop().await.unwrap();
        assert_eq!(segments(&chain.segment_headers()), [(0, 0)]);

        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [c5, c6] {
            task.import(block).await;
        }
        let archived_segment_indices = task.archived_segment_indices();
        task.stop().await.unwrap();

        assert_eq!(archived_segment_indices, [LocalSegmentIndex::ONE]);
        assert_eq!(
            chain.persisted_segment_indices(),
            [LocalSegmentIndex::ZERO, LocalSegmentIndex::ONE]
        );
        assert_eq!(chain.segment_headers(), reference_chain.segment_headers());
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives segments of 128 MiB, too slow under Miri")]
fn continues_after_leaf_shard_block_inserted_by_snap_sync() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = LeafShardBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);
        let c6 = block_builder.child(&c5);
        let c7 = block_builder.child(&c6);
        let c8 = block_builder.child(&c7);

        // Segment 0 ends in the middle of `c2` and segment 1 in the middle of `c5`, which are
        // archived when `c5` and `c8` are about to be imported
        let reference_chain = MockChain::new(genesis_block.clone());
        let mut task = ArchiverTask::start(&reference_chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [&c1, &c2, &c3, &c4, &c5, &c6, &c7, &c8] {
            task.import(block.clone()).await;
        }
        task.stop().await.unwrap();
        assert_eq!(
            segments(&reference_chain.segment_headers()),
            [(0, 2), (1, 5)]
        );

        let chain = MockChain::new(genesis_block);
        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();

        // Snap sync stores the header of segment 0, which is the only known segment, then inserts
        // `c2` directly, without its ancestors:
        //
        // genesis   (missing)   c2 - c3 - .. - c8
        chain
            .persist_segment_headers(vec![
                reference_chain
                    .get_segment_header(LocalSegmentIndex::ZERO)
                    .unwrap(),
            ])
            .await
            .unwrap();
        chain.add_block(c2);

        // Blocks below `c2` are skipped, segment 0 ends in `c2` and is found by re-initialization
        // when `c5` is about to be imported. Archiving continues from the part of `c2` that
        // segment 0 doesn't contain, and segment 1 is the same as without Snap sync:
        //
        // block being imported  c3  c4  c5  c6  c7  c8
        // block to archive       0   1   2   3   4   5
        //                        \---/   |   \-------/
        //                       skipped  |    archived
        //                                found by re-initialization
        // segment archived                           1
        for block in [c3, c4, c5, c6, c7, c8] {
            task.import(block).await;
        }
        let archived_segment_indices = task.archived_segment_indices();
        task.stop().await.unwrap();

        assert_eq!(archived_segment_indices, [LocalSegmentIndex::ONE]);
        assert_eq!(chain.segment_headers(), reference_chain.segment_headers());
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives segments of 128 MiB, too slow under Miri")]
fn restart_archives_from_genesis_when_first_segment_ends_above_blocks_to_archive() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = LeafShardBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);

        // Segment 0 ends in the middle of `c2`, which is archived when `c5` is about to be imported
        let chain = MockChain::new(genesis_block.clone());
        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [&c1, &c2, &c3, &c4, &c5] {
            task.import(block.clone()).await;
        }
        task.stop().await.unwrap();
        assert_eq!(segments(&chain.segment_headers()), [(0, 2)]);

        // After a restart `c5` is gone, so `c2` is above the blocks that re-initialization
        // archives, and it can't resume from segment 0. It archives blocks up to `c1` from the
        // genesis block instead, and segment 0 is created and announced again when `c5` is
        // imported again:
        //
        // block being imported  c5
        // block to archive      c2
        // segment announced      0
        let chain_after_restart = MockChain::new(genesis_block);
        for block in [c1, c2, c3, c4] {
            chain_after_restart.add_block(block);
        }
        chain_after_restart
            .persist_segment_headers(chain.segment_headers())
            .await
            .unwrap();
        let mut task = ArchiverTask::start(&chain_after_restart, block_confirmation_depth)
            .await
            .unwrap();
        task.import(c5).await;
        let archived_segment_indices = task.archived_segment_indices();
        task.stop().await.unwrap();

        assert_eq!(archived_segment_indices, [LocalSegmentIndex::ZERO]);
        assert_eq!(
            chain_after_restart.segment_headers(),
            chain.segment_headers()
        );
    });
}

#[test]
#[cfg_attr(miri, ignore = "Archives segments of 128 MiB, too slow under Miri")]
fn restart_announces_recreated_segment_again() {
    run(async {
        let block_confirmation_depth = BlockNumber::from(3);
        let block_builder = BeaconChainBlockBuilder::new(LARGE_BODY_SIZE);
        let genesis_block = block_builder.genesis();
        let c1 = block_builder.child(&genesis_block);
        let c2 = block_builder.child(&c1);
        let c3 = block_builder.child(&c2);
        let c4 = block_builder.child(&c3);
        let c5 = block_builder.child(&c4);
        let c6 = block_builder.child(&c5);
        let c7 = block_builder.child(&c6);
        let c8 = block_builder.child(&c7);
        let c9 = block_builder.child(&c8);

        // Segment 1 ends in `c3`, which is archived when `c6` is about to be imported
        let chain = MockChain::new(genesis_block.clone());
        let mut task = ArchiverTask::start(&chain, block_confirmation_depth)
            .await
            .unwrap();
        for block in [&c1, &c2, &c3, &c4, &c5, &c6] {
            task.import(block.clone()).await;
        }
        assert_eq!(
            task.archived_segment_indices(),
            [LocalSegmentIndex::from(1)]
        );
        task.stop().await.unwrap();

        // After a restart `c6` is gone, for example because the node stopped before it was
        // persisted, but segment 1 is stored already. When `c6` is imported again, segment 1 is
        // created and announced again. Segment 2 ends in `c6`, which is archived when `c9` is about
        // to be imported:
        //
        // block being imported  c6  c7  c8  c9
        // block to archive      c3  c4  c5  c6
        // segment announced      1           2
        let chain_after_restart = MockChain::new(genesis_block);
        for block in [&c1, &c2, &c3, &c4, &c5] {
            chain_after_restart.add_block(block.clone());
        }
        chain_after_restart
            .persist_segment_headers(chain.segment_headers())
            .await
            .unwrap();
        let mut task = ArchiverTask::start(&chain_after_restart, block_confirmation_depth)
            .await
            .unwrap();
        for block in [c6, c7, c8, c9] {
            task.import(block).await;
        }
        assert_eq!(
            task.archived_segment_indices(),
            [LocalSegmentIndex::from(1), LocalSegmentIndex::from(2)]
        );
        assert_eq!(
            segments(&chain_after_restart.segment_headers()),
            [(0, 0), (1, 3), (2, 6)]
        );
        task.stop().await.unwrap();
    });
}
