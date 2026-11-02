use crate::consensus_parameters::derive_super_segments_for_block;
use crate::test_utils::MockBeaconChain;
use ab_client_api::{ChainInfo, ShardSegmentRoot};
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::segments::{LocalSegmentIndex, SegmentRoot, SuperSegmentRoot};
use ab_core_primitives::shard::ShardIndex;

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
/// `parent_number`
fn derive(chain: &MockBeaconChain, parent_number: u64) -> Option<(SuperSegmentRoot, Vec<u64>)> {
    derive_super_segments_for_block(
        chain,
        BlockNumber::from(parent_number),
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
