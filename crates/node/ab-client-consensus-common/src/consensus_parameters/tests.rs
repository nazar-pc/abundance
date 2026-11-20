use crate::consensus_parameters::{
    CheckShardSegmentsError, derive_super_segment_for_block, shard_membership_entropy_source,
};
use crate::test_utils::MockBeaconChain;
use ab_client_api::{ChainInfo, ShardSegmentRoot};
use ab_core_primitives::block::body::OwnSegments;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{LocalSegmentIndex, SegmentRoot, SuperSegmentRoot};
use ab_core_primitives::shard::ShardIndex;
use std::iter;
use std::ops::Range;

const BLOCK_CONFIRMATION_DEPTH: u64 = 5;
const SHARD_CONFIRMATION_DEPTH: u64 = 2;

/// Segment roots of shard `1` with `segment_indices`, `salt` makes them unique per chain
fn shard_segment_roots(segment_indices: &[u64], salt: u8) -> Vec<ShardSegmentRoot> {
    segment_indices
        .iter()
        .map(|&segment_index| ShardSegmentRoot {
            shard_index: ShardIndex::new(1).unwrap(),
            segment_index: LocalSegmentIndex::from(segment_index),
            segment_root: SegmentRoot::from([u8::try_from(segment_index).unwrap() ^ salt; _]),
        })
        .collect()
}

/// Add blocks with `segment_indices` of shard `1` on top of `parent_root` and return their roots
fn add_chain(
    chain: &mut MockBeaconChain,
    mut parent_root: BlockRoot,
    segment_indices: &[&[u64]],
    salt: u8,
) -> Vec<BlockRoot> {
    segment_indices
        .iter()
        .map(|segment_indices| {
            parent_root = chain.add_block(parent_root, shard_segment_roots(segment_indices, salt));
            parent_root
        })
        .collect()
}

/// Root and local segment indices of the super segment derived for the child of the block at
/// `parent_number` on the best chain
fn derive(chain: &MockBeaconChain, parent_number: u64) -> Option<(SuperSegmentRoot, Vec<u64>)> {
    let parent_number = BlockNumber::from(parent_number);
    let parent_header = chain
        .ancestor_header(parent_number, &chain.best_root())
        .unwrap();
    derive_super_segment_for_block(
        chain,
        parent_number,
        &parent_header.header().root(),
        BlockNumber::from(BLOCK_CONFIRMATION_DEPTH),
        BlockNumber::from(SHARD_CONFIRMATION_DEPTH),
    )
    .unwrap()
    .map(|super_segment| {
        let segment_indices = super_segment
            .segment_roots
            .iter()
            .map(|segment_root| u64::from(segment_root.local_segment_index))
            .collect();

        (super_segment.header.root, segment_indices)
    })
}

/// Chain with blocks that include `segment_indices` of shard `1` on top of genesis and roots of all
/// its blocks, the block number is the position in the list
fn chain_with_blocks(segment_indices: &[&[u64]]) -> (MockBeaconChain, Vec<BlockRoot>) {
    let mut chain = MockBeaconChain::new();
    let genesis_root = chain.best_root();
    let blocks = iter::once(genesis_root)
        .chain(add_chain(&mut chain, genesis_root, segment_indices, 0))
        .collect();

    (chain, blocks)
}

/// Reason a shard segment is rejected for
#[derive(Debug, PartialEq, Eq)]
enum Rejected {
    /// Already confirmed, with the highest confirmed local segment index
    AlreadyConfirmed(u64),
    /// Doesn't continue the shard segments listed before it, with the next local segment index
    NotContinuous(u64),
}

/// Check own segments listed in block `block_number` with parent `parent_root`, each with its shard
/// index and local segment indices.
///
/// Returns the shard index and the first local segment index of the rejected own segments with the
/// reason.
fn check_shard_segments(
    chain: &MockBeaconChain,
    block_number: u64,
    parent_root: &BlockRoot,
    shard_segments: &[(ShardIndex, Range<u64>)],
) -> Option<(ShardIndex, u64, Rejected)> {
    let segment_roots = [SegmentRoot::default(); 8];
    let shard_segments = shard_segments
        .iter()
        .map(|(shard_index, local_segment_indices)| {
            let own_segments = OwnSegments::new(
                LocalSegmentIndex::from(local_segment_indices.start),
                &segment_roots[..local_segment_indices.clone().count()],
            )
            .unwrap();
            (*shard_index, own_segments)
        });

    match crate::consensus_parameters::check_shard_segments(
        chain,
        BlockNumber::from(block_number),
        parent_root,
        shard_segments,
        BlockNumber::from(BLOCK_CONFIRMATION_DEPTH),
        BlockNumber::from(SHARD_CONFIRMATION_DEPTH),
    ) {
        Ok(()) => None,
        Err(CheckShardSegmentsError::ShardSegmentAlreadyConfirmed {
            shard_index,
            local_segment_index,
            max_confirmed_local_segment_index,
        }) => Some((
            shard_index,
            u64::from(local_segment_index),
            Rejected::AlreadyConfirmed(u64::from(max_confirmed_local_segment_index)),
        )),
        Err(CheckShardSegmentsError::ShardSegmentNotContinuous {
            shard_index,
            local_segment_index,
            next_local_segment_index,
        }) => Some((
            shard_index,
            u64::from(local_segment_index),
            Rejected::NotContinuous(u64::from(next_local_segment_index)),
        )),
        Err(error) => panic!("Unexpected error: {error}"),
    }
}

/// Check segment `local_segment_index` of `shard_index` listed in block `block_number` on top of
/// `blocks`.
///
/// Returns the reason if the segment is rejected.
fn check_shard_segment(
    chain: &MockBeaconChain,
    blocks: &[BlockRoot],
    block_number: u64,
    shard_index: ShardIndex,
    local_segment_index: u64,
) -> Option<Rejected> {
    check_shard_segments(
        chain,
        block_number,
        &blocks[usize::try_from(block_number - 1).unwrap()],
        &[(shard_index, local_segment_index..local_segment_index + 1)],
    )
    .map(
        |(rejected_shard_index, rejected_local_segment_index, rejected)| {
            assert_eq!(rejected_shard_index, shard_index);
            assert_eq!(rejected_local_segment_index, local_segment_index);

            rejected
        },
    )
}

#[test]
fn super_segment_of_deepest_fork_is_derived_from_own_ancestry() {
    // genesis - c1 .. c4 -+- c5 .. c8 (best block)
    //                     \- a5 .. a8 (`a9` on top reorgs `block_confirmation_depth - 1` blocks)
    //
    // The super segment of `a9` has segments of `c2` except those that `c3 ..= c4` include again
    // (a shard reorg). `c5` includes segments of both `c2` and `c3` again, so reading it instead of
    // `a5` (one block too shallow) changes the super segment, whether only the end or the whole
    // range of blocks read is shifted.
    let mut chain = MockBeaconChain::new();
    let genesis_root = chain.best_root();
    let canonical_chain = add_chain(
        &mut chain,
        genesis_root,
        &[
            &[],
            &[7, 8, 9],
            &[10, 11, 12],
            &[9],
            &[8, 9, 10],
            &[],
            &[],
            &[],
        ],
        0,
    );
    let fork_chain = add_chain(&mut chain, canonical_chain[3], &[&[14], &[], &[], &[]], 1);

    chain.set_best(*canonical_chain.last().unwrap());
    let derived = derive(&chain, 8);
    chain.set_best(*fork_chain.last().unwrap());
    let derived_from_own_ancestry = derive(&chain, 8);

    assert_eq!(
        derived
            .as_ref()
            .map(|(_root, segment_indices)| segment_indices.as_slice()),
        Some([7, 8].as_slice())
    );
    assert_eq!(derived, derived_from_own_ancestry);
}

#[test]
fn shard_reorg_below_first_segment_postpones_all_segments() {
    // genesis - c1 .. c10 (best block)
    //
    // `c4` includes segment `9` again (a shard reorg), which `c3` doesn't include, but its segments
    // `10 ..= 12` follow `9`, so they are postponed along with it and `c10` gets no super segment
    let mut chain = MockBeaconChain::new();
    let genesis_root = chain.best_root();
    let canonical_chain = add_chain(
        &mut chain,
        genesis_root,
        &[
            &[],
            &[7, 8, 9],
            &[10, 11, 12],
            &[9],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
        ],
        0,
    );
    chain.set_best(*canonical_chain.last().unwrap());

    let segment_indices = |parent_number| {
        derive(&chain, parent_number).map(|(_root, segment_indices)| segment_indices)
    };
    assert_eq!(segment_indices(8), Some(vec![7, 8]));
    assert_eq!(segment_indices(9), None);
    assert_eq!(segment_indices(10), Some(vec![9]));
}

#[test]
fn non_contiguous_shard_segments_are_kept() {
    // genesis - c1 .. c8 (best block)
    //
    // `c2` includes segments `7` and `9` without `8`, both are kept because nothing includes either
    // of them again
    let mut chain = MockBeaconChain::new();
    let genesis_root = chain.best_root();
    let canonical_chain = add_chain(
        &mut chain,
        genesis_root,
        &[&[], &[7, 9], &[], &[], &[], &[], &[], &[]],
        0,
    );
    chain.set_best(*canonical_chain.last().unwrap());

    assert_eq!(
        derive(&chain, 8).map(|(_root, segment_indices)| segment_indices),
        Some(vec![7, 9])
    );
}

#[test]
fn shard_membership_entropy_is_taken_from_own_ancestry() {
    // genesis - c1 .. c3 -+- c4 .. c10 (best block)
    //                     \- a4 .. a9
    //
    // With these shard rotation parameters the entropy source of block numbers 10 and 11 is block
    // number 8, so `a10` on top of `a9` must use `a8` rather than `c8` on the best chain
    let shard_rotation_interval = BlockNumber::from(4);
    let shard_rotation_delay = BlockNumber::from(2);
    let mut chain = MockBeaconChain::new();
    let genesis_root = chain.best_root();
    let canonical_chain = add_chain(&mut chain, genesis_root, &[[].as_slice(); 10], 0);
    let fork_chain = add_chain(&mut chain, canonical_chain[2], &[[].as_slice(); 6], 0);
    chain.set_best(*canonical_chain.last().unwrap());
    let c8 = canonical_chain[7];
    let c10 = canonical_chain[9];
    let a8 = fork_chain[4];
    let a9 = fork_chain[5];

    let entropy = |block_number, parent_block_root| {
        shard_membership_entropy_source(
            BlockNumber::from(block_number),
            parent_block_root,
            shard_rotation_interval,
            shard_rotation_delay,
            &chain,
        )
        .unwrap()
    };

    assert_eq!(
        entropy(10, &a9),
        chain.proof_of_time(&a8).shard_membership_entropy()
    );
    assert_eq!(
        entropy(11, &c10),
        chain.proof_of_time(&c8).shard_membership_entropy()
    );
    assert_ne!(
        chain.proof_of_time(&a8).shard_membership_entropy(),
        chain.proof_of_time(&c8).shard_membership_entropy()
    );
}

#[test]
fn shard_segment_at_or_below_persisted_maximum_is_rejected() {
    let shard_index = ShardIndex::new(1).unwrap();
    let (mut chain, blocks) = chain_with_blocks(&[[].as_slice(); 10]);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(7));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 6),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 7),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 8),
        None
    );
}

#[test]
fn confirmed_shard_segment_that_is_not_persisted_yet_is_rejected() {
    // Segment `7` of block 10 is not included again in blocks 11 and 12, so it is confirmed for
    // blocks from 13 on, but only becomes a part of the super segment of block 17
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 16];
    segment_indices[9] = &[7];
    let (chain, blocks) = chain_with_blocks(&segment_indices);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, shard_index, 7),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 14, shard_index, 7),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 17, shard_index, 7),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, shard_index, 8),
        None
    );
}

#[test]
fn shard_segment_included_again_before_confirmation_is_accepted() {
    // Segment `7` of block 10 is not confirmed until block 12 is known to not include it again
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 11];
    segment_indices[9] = &[7];
    let (chain, blocks) = chain_with_blocks(&segment_indices);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 7),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 12, shard_index, 7),
        None
    );
}

#[test]
fn shard_segment_of_new_shard_is_accepted() {
    let shard_index = ShardIndex::new(1).unwrap();
    let new_shard_index = ShardIndex::new(2).unwrap();
    let mut segment_indices = [[].as_slice(); 12];
    segment_indices[9] = &[7];
    let (mut chain, blocks) = chain_with_blocks(&segment_indices);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, shard_index, 7),
        Some(Rejected::AlreadyConfirmed(7))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, new_shard_index, 0),
        None
    );
}

#[test]
fn every_listed_shard_segment_is_checked() {
    let shard_index = ShardIndex::new(1).unwrap();
    let new_shard_index = ShardIndex::new(2).unwrap();
    let mut segment_indices = [[].as_slice(); 12];
    segment_indices[9] = &[7];
    let (chain, blocks) = chain_with_blocks(&segment_indices);

    assert_eq!(
        check_shard_segments(
            &chain,
            13,
            &blocks[12],
            &[(new_shard_index, 0..1), (shard_index, 7..8)]
        ),
        Some((shard_index, 7, Rejected::AlreadyConfirmed(7)))
    );
}

#[test]
fn confirmed_shard_segments_are_taken_from_own_ancestry() {
    // genesis - b1 .. b9 -+- a10 .. a12 (best block)
    //                     \- c10 .. c12
    //
    // Only `a10` includes segment `7`, so it is confirmed on top of `a12`, but not on top of `c12`
    let shard_index = ShardIndex::new(1).unwrap();
    let (mut chain, blocks) = chain_with_blocks(&[[].as_slice(); 9]);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));
    let a = add_chain(&mut chain, blocks[9], &[&[7], &[], &[]], 0);
    let c = add_chain(&mut chain, blocks[9], &[&[], &[], &[]], 1);
    chain.set_best(a[2]);

    assert_eq!(
        check_shard_segments(&chain, 13, &a[2], &[(shard_index, 7..8)]),
        Some((shard_index, 7, Rejected::AlreadyConfirmed(7)))
    );
    assert_eq!(
        check_shard_segments(&chain, 13, &c[2], &[(shard_index, 7..8)]),
        None
    );
}

#[test]
fn postponed_shard_segment_is_not_confirmed() {
    // Block 11 includes segment `9` of block 10 again (a shard reorg), so only segments `7` and `8`
    // of block 10 are confirmed for block 13, while segment `9` of block 11 is confirmed for block
    // 14
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 13];
    segment_indices[9] = &[7, 8, 9];
    segment_indices[10] = &[9];
    let (chain, blocks) = chain_with_blocks(&segment_indices);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, shard_index, 8),
        Some(Rejected::AlreadyConfirmed(8))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 13, shard_index, 9),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 14, shard_index, 9),
        Some(Rejected::AlreadyConfirmed(9))
    );
}

#[test]
fn shard_segments_of_early_blocks_are_checked() {
    // Segment `0` of block 1 is the first one that can be confirmed, for blocks from 4 on
    let shard_index = ShardIndex::new(1).unwrap();
    let (chain, blocks) = chain_with_blocks(&[&[0], &[], &[]]);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 1, shard_index, 0),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 2, shard_index, 0),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 3, shard_index, 0),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 4, shard_index, 0),
        Some(Rejected::AlreadyConfirmed(0))
    );
}

#[test]
fn shard_segments_of_new_shard_start_at_zero() {
    let shard_index = ShardIndex::new(1).unwrap();
    let (chain, blocks) = chain_with_blocks(&[[].as_slice(); 10]);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 1),
        Some(Rejected::NotContinuous(0))
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 0),
        None
    );
}

#[test]
fn shard_segments_continue_persisted_maximum() {
    let shard_index = ShardIndex::new(1).unwrap();
    let (mut chain, blocks) = chain_with_blocks(&[[].as_slice(); 10]);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(7));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 8),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 9),
        Some(Rejected::NotContinuous(8))
    );
}

#[test]
fn shard_segments_continue_most_recent_listing() {
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 10];
    segment_indices[9] = &[7, 8, 9];
    let (mut chain, blocks) = chain_with_blocks(&segment_indices);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 10),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 11),
        Some(Rejected::NotContinuous(10))
    );
}

#[test]
fn shard_segments_listed_again_after_shard_reorg_are_accepted() {
    // Segments of block 10 are not confirmed yet, so a shard reorg can list them again
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 10];
    segment_indices[9] = &[7, 8, 9];
    let (mut chain, blocks) = chain_with_blocks(&segment_indices);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 8),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 7),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 11, shard_index, 6),
        Some(Rejected::AlreadyConfirmed(6))
    );
}

#[test]
fn most_recent_listing_of_shard_segments_is_continued() {
    // Block 11 lists segment `8` of block 10 again, so the next segment is `9` rather than `10`
    let shard_index = ShardIndex::new(1).unwrap();
    let mut segment_indices = [[].as_slice(); 11];
    segment_indices[9] = &[7, 8, 9];
    segment_indices[10] = &[8];
    let (mut chain, blocks) = chain_with_blocks(&segment_indices);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));

    assert_eq!(
        check_shard_segment(&chain, &blocks, 12, shard_index, 9),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 12, shard_index, 10),
        Some(Rejected::NotContinuous(9))
    );
}

#[test]
fn own_segments_of_shard_in_one_block_are_contiguous() {
    let shard_index = ShardIndex::new(1).unwrap();
    let new_shard_index = ShardIndex::new(2).unwrap();
    let (mut chain, blocks) = chain_with_blocks(&[[].as_slice(); 10]);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(4));

    assert_eq!(
        check_shard_segments(
            &chain,
            11,
            &blocks[10],
            &[
                (shard_index, 5..8),
                (new_shard_index, 0..1),
                (shard_index, 8..9)
            ]
        ),
        None
    );
    assert_eq!(
        check_shard_segments(
            &chain,
            11,
            &blocks[10],
            &[(shard_index, 5..8), (shard_index, 6..7)]
        ),
        Some((shard_index, 6, Rejected::NotContinuous(8)))
    );
    assert_eq!(
        check_shard_segments(
            &chain,
            11,
            &blocks[10],
            &[(shard_index, 5..8), (shard_index, 9..10)]
        ),
        Some((shard_index, 9, Rejected::NotContinuous(8)))
    );
}

#[test]
fn shard_segments_of_early_blocks_are_continuous() {
    let shard_index = ShardIndex::new(1).unwrap();
    let (chain, blocks) = chain_with_blocks(&[&[]]);

    assert_eq!(
        check_shard_segment(&chain, &blocks, 2, shard_index, 0),
        None
    );
    assert_eq!(
        check_shard_segment(&chain, &blocks, 2, shard_index, 3),
        Some(Rejected::NotContinuous(0))
    );
}

#[test]
fn next_shard_segments_are_taken_from_own_ancestry() {
    // genesis - b1 .. b9 -+- a10 (best block)
    //                     \- c10
    //
    // Only `a10` includes segment `7`, so segment `8` continues it on top of `a10`, but skips
    // segment `7` on top of `c10`
    let shard_index = ShardIndex::new(1).unwrap();
    let (mut chain, blocks) = chain_with_blocks(&[[].as_slice(); 9]);
    chain.set_shard_max_local_segment_index(shard_index, LocalSegmentIndex::from(6));
    let a = add_chain(&mut chain, blocks[9], &[&[7]], 0);
    let c = add_chain(&mut chain, blocks[9], &[&[]], 1);
    chain.set_best(a[0]);

    assert_eq!(
        check_shard_segments(&chain, 11, &a[0], &[(shard_index, 8..9)]),
        None
    );
    assert_eq!(
        check_shard_segments(&chain, 11, &c[0], &[(shard_index, 8..9)]),
        Some((shard_index, 8, Rejected::NotContinuous(7)))
    );
}
