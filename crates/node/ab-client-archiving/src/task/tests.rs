use crate::test_utils::{ArchiverTask, BeaconChainBlockBuilder, MockChain, run};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::segments::{LocalSegmentIndex, RecordedHistorySegment, SegmentHeader};

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
