use crate::block::BlockNumber;
use crate::hashes::Blake3Hash;
use crate::segments::{
    LocalSegmentIndex, SegmentIndex, SegmentPosition, SegmentRoot, ShardSegmentRootWithPosition,
    SuperSegment, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use crate::shard::ShardIndex;
use std::sync::Arc;

/// Zero padding of segment proofs must be at the end, so that a proof has a single encoding
#[test]
fn segment_proofs_only_have_padding_at_the_end() {
    let previous_header = SuperSegmentHeader {
        index: SuperSegmentIndex::ZERO.into(),
        root: SuperSegmentRoot::default(),
        prev_super_segment_header_hash: Blake3Hash::default(),
        max_segment_index: SegmentIndex::ZERO.into(),
        target_beacon_chain_block_number: BlockNumber::ZERO.into(),
        num_segments: 0,
    };
    let segment_roots = (0..5_u8)
        .map(|position| ShardSegmentRootWithPosition {
            shard_index: ShardIndex::BEACON_CHAIN,
            segment_position: SegmentPosition::from(u32::from(position)),
            local_segment_index: LocalSegmentIndex::from(u64::from(position)),
            segment_root: SegmentRoot::from([position + 1; _]),
        })
        .collect::<Arc<[_]>>();
    let super_segment = SuperSegment::new(
        &previous_header,
        BlockNumber::ONE,
        Arc::clone(&segment_roots),
    )
    .unwrap();

    for shard_segment_root in segment_roots.iter() {
        let is_valid = |segment_proof| {
            shard_segment_root.segment_root.is_valid(
                shard_segment_root.shard_index,
                shard_segment_root.local_segment_index,
                shard_segment_root.segment_position,
                &segment_proof,
                super_segment.header.num_segments,
                &super_segment.header.root,
            )
        };

        let segment_proof = super_segment
            .proof_for_segment(shard_segment_root.segment_position)
            .unwrap();
        assert!(is_valid(segment_proof));

        let proof_length = segment_proof
            .iter()
            .position(|hash| hash == &[0; _])
            .unwrap();
        // Something other than zeroes after the padding starts
        for position in proof_length + 1..segment_proof.len() {
            let mut segment_proof = segment_proof;
            segment_proof[position] = [1; _];
            assert!(!is_valid(segment_proof));
        }
        // An extra hash instead of padding
        let mut segment_proof = segment_proof;
        segment_proof[proof_length] = [1; _];
        assert!(!is_valid(segment_proof));
    }
}

#[test]
fn super_segment_header_segment_position() {
    let header = |max_segment_index: u64, num_segments: u32| SuperSegmentHeader {
        index: SuperSegmentIndex::ONE.into(),
        root: SuperSegmentRoot::default(),
        prev_super_segment_header_hash: Blake3Hash::default(),
        max_segment_index: SegmentIndex::from(max_segment_index).into(),
        target_beacon_chain_block_number: BlockNumber::ONE.into(),
        num_segments,
    };
    let segment_position = |header: SuperSegmentHeader, segment_index: u64| {
        header
            .segment_position(SegmentIndex::from(segment_index))
            .map(u32::from)
    };

    // Genesis super segment
    let genesis_header = header(0, 1);
    assert_eq!(segment_position(genesis_header, 0), Some(0));
    assert_eq!(segment_position(genesis_header, 1), None);

    let header_10_3 = header(10, 3);
    assert_eq!(segment_position(header_10_3, 7), None);
    assert_eq!(segment_position(header_10_3, 8), Some(0));
    assert_eq!(segment_position(header_10_3, 9), Some(1));
    assert_eq!(segment_position(header_10_3, 10), Some(2));
    assert_eq!(segment_position(header_10_3, 11), None);
    assert_eq!(segment_position(header_10_3, u64::MAX), None);

    // Largest values
    let header_max = header(u64::MAX, u32::MAX);
    assert_eq!(segment_position(header_max, u64::MAX), Some(u32::MAX - 1));
    let first_segment_index = u64::MAX - u64::from(u32::MAX - 1);
    assert_eq!(segment_position(header_max, first_segment_index), Some(0));
    assert_eq!(segment_position(header_max, first_segment_index - 1), None);
    assert_eq!(segment_position(header_max, 0), None);

    // Headers that can't be produced by a super segment don't cause overflows: an empty one
    // contains nothing, and one with more segments than indices up to the max segment index
    // contains segments from zero at the last positions
    for max_segment_index in [0, 5, u64::MAX] {
        for segment_index in [0, 5, u64::MAX] {
            assert_eq!(
                segment_position(header(max_segment_index, 0), segment_index),
                None
            );
        }
    }
    let header_1_5 = header(1, 5);
    assert_eq!(segment_position(header_1_5, 0), Some(3));
    assert_eq!(segment_position(header_1_5, 1), Some(4));
    assert_eq!(segment_position(header_1_5, 2), None);
}
