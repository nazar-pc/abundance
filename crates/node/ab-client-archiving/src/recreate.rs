//! Segment re-creation

use crate::task::encode_block;
use ab_archiving::archiver::{Archiver, ArchiverInstantiationError, NewArchivedSegment};
use ab_archiving::objects::BlockObject;
use ab_client_api::{ChainInfo, ReadBlockError};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBeaconChainBlock};
use ab_core_primitives::pieces::SegmentProof;
use ab_core_primitives::segments::{SegmentHeader, SegmentPosition, SuperSegmentIndex};
use ab_core_primitives::shard::ShardIndex;
use ab_erasure_coding::ErasureCoding;
use tokio::task::{JoinError, spawn_blocking};

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

/// Error for [`recreate_segment()`]
#[derive(Debug, thiserror::Error)]
pub enum RecreateSegmentError {
    /// Read block error
    #[error("Read block error: {0}")]
    ReadBlockError(#[from] ReadBlockError),
    /// Archiver instantiation error
    #[error("Archiver instantiation error: {0}")]
    ArchiverInstantiationError(#[from] ArchiverInstantiationError),
    /// Failed to add block to the archiver
    #[error("Failed to add block to the archiver")]
    FailedToAddBlock,
    /// Blocking task join error
    #[error("Blocking task join error: {0}")]
    BlockingTaskJoinError(#[from] JoinError),
}

/// Super segment details for [`recreate_segment()`]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct RecreateSegmentSuperSegmentDetails {
    /// Super segment index
    pub super_segment_index: SuperSegmentIndex,
    /// Segment position in a super segment
    pub segment_position: SegmentPosition,
    /// Segment proof
    pub segment_proof: SegmentProof,
}

/// Re-create a segment on demand.
///
/// `last_segment_header` corresponds to the last segment before the segment being re-created and
/// indicates where segment archiving should start.
///
/// `extract_block_objects` allows extracting objects stored in blocks to translate them into global
/// objects.
///
/// Returns `Ok(None)` if one of the segment blocks is already pruned.
pub async fn recreate_segment<Block, CI, EBO>(
    last_segment_header: Option<SegmentHeader>,
    chain_info: &CI,
    erasure_coding: ErasureCoding,
    super_segment_details: &RecreateSegmentSuperSegmentDetails,
    mut extract_block_objects: EBO,
) -> Result<Option<NewArchivedSegment>, RecreateSegmentError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfo<Block>,
    EBO: FnMut(&Block) -> Vec<BlockObject>,
{
    let best_block_root = chain_info.best_root();

    let (start_block_number, mut archiver) = if let Some(last_segment_header) = last_segment_header
    {
        let first_block_number = last_segment_header.last_archived_block.number.as_inner();

        let archiver = {
            let Some(first_block_root) = chain_info
                .ancestor_header(first_block_number, &best_block_root)
                .map(|header| *header.header().root())
            else {
                return Ok(None);
            };
            let block = chain_info.block(&first_block_root).await?;
            let shard_index = block.header().header().prefix.shard_index;
            let encoded_block = encode_block(&block);

            Archiver::with_initial_state(
                shard_index,
                erasure_coding,
                last_segment_header,
                &encoded_block,
                extract_block_objects(&block),
            )?
        };

        (
            first_block_number.saturating_sub(BlockNumber::ONE),
            archiver,
        )
    } else {
        let best_header = chain_info.best_header();
        let archiver = Archiver::new(best_header.header().prefix.shard_index, erasure_coding);

        (BlockNumber::ZERO, archiver)
    };

    for block_number in start_block_number.. {
        let (encoded_block, block_objects) = {
            let Some(block_root) = chain_info
                .ancestor_header(block_number, &best_block_root)
                .map(|header| *header.header().root())
            else {
                return Ok(None);
            };
            let block = chain_info.block(&block_root).await?;

            (encode_block(&block), extract_block_objects(&block))
        };

        let task_fut = spawn_blocking(move || {
            let maybe_outcome = archiver
                .add_block(encoded_block, block_objects)
                .ok_or(RecreateSegmentError::FailedToAddBlock);

            (archiver, maybe_outcome)
        });
        let outcome;
        (archiver, outcome) = task_fut.await?;
        let outcome = outcome?;

        // TODO: Return global objects once archiver API improves
        if let Some(mut archived_segment) = outcome.archived_segments.into_iter().next() {
            for piece in archived_segment.pieces.iter_mut() {
                piece
                    .header
                    .super_segment_index
                    .replace(super_segment_details.super_segment_index);
                piece
                    .header
                    .segment_position
                    .replace(super_segment_details.segment_position);
                piece
                    .header
                    .segment_proof
                    .copy_from_slice(super_segment_details.segment_proof.as_slice());
            }

            return Ok(Some(archived_segment));
        }
    }

    Ok(None)
}
