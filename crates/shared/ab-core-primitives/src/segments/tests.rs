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
