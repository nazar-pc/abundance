//! Segment re-creation

use crate::task::encode_block;
use ab_archiving::archiver::{Archiver, NewArchivedSegment};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::segments::SuperSegmentIndex;
use ab_core_primitives::shard::ShardIndex;
use ab_erasure_coding::ErasureCoding;

/// Re-create the genesis segment on demand.
///
/// This is a bit of a hack and is useful for deriving of the genesis beacon chain segment that is a
/// special case since we don't have enough data in the blockchain history itself during genesis to
/// do the archiving.
pub fn recreate_genesis_segment(
    owned_genesis_block: &OwnedBeaconChainBlock,
    erasure_coding: ErasureCoding,
) -> NewArchivedSegment {
    let encoded_block = encode_block(owned_genesis_block);

    let block_outcome = Archiver::new(ShardIndex::BEACON_CHAIN, erasure_coding)
        .add_block(encoded_block, Vec::new())
        .expect("Block is never empty and doesn't exceed u32; qed");
    let mut archived_segment = block_outcome
        .archived_segments
        .into_iter()
        .next()
        .expect("Genesis block always results in exactly one archived segment; qed");

    for piece in archived_segment.pieces.iter_mut() {
        piece.header.super_segment_index = SuperSegmentIndex::ZERO.into();
        // Since there is a single segment in super segment, the proof is empty
    }

    archived_segment.pieces = archived_segment.pieces.to_shared();

    archived_segment
}
