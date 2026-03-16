//! Segment archiving task.
//!
//! Implements archiving process in that converts blockchain history (blocks) into archived history
//! (segments and pieces).
//!
//! The main entry point here is [`create_segment_archiver_task`] that will create a task, which
//! while driven will perform the archiving itself.
//!
//! Archiving itself will also wait for acknowledgement by various subscribers before proceeding,
//! which includes farmer subscription, in case of reference implementation via RPC.
//!
//! All segment headers of the archived segments are available to other parts of the protocol that
//! need to know what the correct archival history of the blockchain looks like through
//! [`ChainInfo`]. For example, it is used during node sync and farmer plotting to verify pieces of
//! archival history received from other network participants. Future segment header might also be
//! already known in the case of syncing from DSN.
//!
//! [`encode_block`] and [`decode_block`] are symmetric encoding/decoding functions turning
//! Blocks into bytes and back.

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_archiving::archiver::{Archiver, ArchiverInstantiationError, NewArchivedSegment};
use ab_client_api::{ChainInfo, ChainInfoWrite, PersistSegmentHeadersError};
use ab_client_consensus_common::{BlockImportingNotification, ConsensusConstants};
use ab_core_primitives::block::body::owned::GenericOwnedBlockBody;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, GenericBlock};
use ab_core_primitives::segments::{LocalSegmentIndex, RecordedHistorySegment, SegmentHeader};
use ab_core_primitives::shard::RealShardKind;
use ab_erasure_coding::ErasureCoding;
use bytesize::ByteSize;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use futures::channel::mpsc;
use futures::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, trace, warn};

/// Do not wait for acknowledgements beyond this time limit
const ACKNOWLEDGEMENT_TIMEOUT: Duration = Duration::from_mins(2);

// TODO: Maybe use or remove if database handles this completely on its own
// /// How deep (in segments) should block be in order to be finalized.
// ///
// /// This is required for full nodes to not prune recent history such that keep-up sync in
// /// Substrate works even without archival nodes (initial sync will be done from DSN).
// ///
// /// Ideally, we'd decouple pruning from finalization, but it may require invasive changes in
// /// Substrate and is not worth it right now.
// /// https://github.com/paritytech/substrate/discussions/14359
// const FINALIZATION_DEPTH_IN_SEGMENTS: SegmentIndex = SegmentIndex::from(5);

/// Notification with a new archived segment that was just archived
#[derive(Debug)]
pub struct ArchivedSegmentNotification {
    /// Archived segment.
    pub archived_segment: Arc<NewArchivedSegment>,
    /// Sender that signified the fact of receiving an archived segment by farmer.
    ///
    /// This must be used to send a message, or else the block import pipeline will get stuck.
    pub acknowledgement_sender: mpsc::Sender<()>,
}

async fn find_last_archived_block<Block, CI>(
    chain_info: &CI,
    best_block_number_to_archive: BlockNumber,
    best_block_root: &BlockRoot,
) -> Option<(SegmentHeader, Block)>
where
    Block: GenericOwnedBlock,
    CI: ChainInfo<Block>,
{
    let max_local_segment_index = chain_info.last_segment_header()?.segment_index.as_inner();

    if max_local_segment_index == LocalSegmentIndex::ZERO {
        // Just genesis, nothing else to check
        return None;
    }

    for segment_header in (LocalSegmentIndex::ZERO..=max_local_segment_index)
        .rev()
        .filter_map(|segment_index| chain_info.get_segment_header(segment_index))
    {
        let last_archived_block_number = segment_header.last_archived_block.number();

        if last_archived_block_number > best_block_number_to_archive {
            // Last archived block in segment header is too high for the current state of the chain
            // (segment headers store may know about more blocks in existence than is currently
            // imported)
            continue;
        }

        let Some(last_archived_block_header) =
            chain_info.ancestor_header(last_archived_block_number, best_block_root)
        else {
            // This block number is not in our chain yet (segment headers store may know about more
            // blocks in existence than is currently imported)
            continue;
        };

        let last_archived_block_root = &*last_archived_block_header.header().root();

        let Ok(last_archived_block) = chain_info.block(last_archived_block_root).await else {
            // Block might have been pruned between ancestor search and disk read
            continue;
        };

        return Some((segment_header, last_archived_block));
    }

    None
}

/// Encode block for archiving purposes
pub fn encode_block<Block>(block: &Block) -> Vec<u8>
where
    Block: GenericOwnedBlock,
{
    let is_beacon_chain_genesis_block = Block::Block::SHARD_KIND == RealShardKind::BeaconChain
        && block.header().header().prefix.number == BlockNumber::ZERO;
    let header_buffer = block.header().buffer();
    let body_buffer = block.body().buffer();

    // TODO: Extra allocation here is unfortunate, would be nice to avoid it
    let mut encoded_block = Vec::with_capacity(if is_beacon_chain_genesis_block {
        RecordedHistorySegment::SIZE
    } else {
        size_of::<u32>() * 2 + header_buffer.len() as usize + body_buffer.len() as usize
    });

    encoded_block.extend_from_slice(&header_buffer.len().to_le_bytes());
    encoded_block.extend_from_slice(&body_buffer.len().to_le_bytes());
    encoded_block.extend_from_slice(header_buffer);
    encoded_block.extend_from_slice(body_buffer);

    if is_beacon_chain_genesis_block {
        let encoded_block_length = encoded_block.len();

        // Encoding of the genesis block is extended with extra data such that the very first
        // archived segment can be produced right away, bootstrapping the farming process.
        //
        // Note: we add it to the end of the encoded block, so during decoding it'll actually be
        // ignored even though it is technically present in encoded form.
        encoded_block.resize(RecordedHistorySegment::SIZE, 0);
        let mut rng = ChaCha8Rng::from_seed(*block.header().header().result.state_root);
        rng.fill_bytes(&mut encoded_block[encoded_block_length..]);
    }

    encoded_block
}

/// Symmetrical to [`encode_block()`], used to decode previously encoded blocks
pub fn decode_block<Block>(mut encoded_block: &[u8]) -> Option<Block>
where
    Block: GenericOwnedBlock,
{
    let header_length = {
        let header_length = encoded_block.split_off(..size_of::<u32>())?;
        u32::from_le_bytes([
            header_length[0],
            header_length[1],
            header_length[2],
            header_length[3],
        ])
    };
    let body_length = {
        let body_length = encoded_block.split_off(..size_of::<u32>())?;
        u32::from_le_bytes([
            body_length[0],
            body_length[1],
            body_length[2],
            body_length[3],
        ])
    };

    let header_buffer = encoded_block.split_off(..header_length as usize)?;
    let body_buffer = encoded_block.split_off(..body_length as usize)?;

    let header_buffer = SharedAlignedBuffer::from_bytes(header_buffer);
    let body_buffer = SharedAlignedBuffer::from_bytes(body_buffer);

    Block::from_buffers(header_buffer, body_buffer)
}

/// Segment archiver task error
#[derive(Debug, thiserror::Error)]
pub enum SegmentArchiverTaskError {
    /// Archiver instantiation error
    #[error("Archiver instantiation error: {error}")]
    Instantiation {
        /// Low-level error
        #[from]
        error: ArchiverInstantiationError,
    },
    /// Failed to persist a new segment header
    #[error("Failed to persist a new segment header: {error}")]
    PersistSegmentHeaders {
        /// Low-level error
        #[from]
        error: PersistSegmentHeadersError,
    },
    /// Attempt to switch to a different fork beyond archiving depth
    #[error(
        "Attempt to switch to a different fork beyond archiving depth: parent block root \
        {parent_block_root}, best archived block root {best_archived_block_root}"
    )]
    ArchivingReorg {
        /// Parent block root
        parent_block_root: BlockRoot,
        /// Best archived block root
        best_archived_block_root: BlockRoot,
    },
    /// There was a gap in blockchain history, and the last contiguous series of blocks doesn't
    /// start with the archived segment
    #[error(
        "There was a gap in blockchain history, and the last contiguous series of blocks doesn't \
        start with the archived segment (best archived block number {best_archived_block_number}, \
        block number to archive {block_number_to_archive}), block about to be imported \
        {importing_block_number})"
    )]
    BlockGap {
        /// Best archived block number
        best_archived_block_number: BlockNumber,
        /// Block number to archive
        block_number_to_archive: BlockNumber,
        /// Importing block number
        importing_block_number: BlockNumber,
    },
}

struct InitializedArchiver {
    archiver: Archiver,
    best_archived_block: (BlockRoot, BlockNumber),
}

async fn initialize_archiver<Block, CI>(
    chain_info: &CI,
    block_confirmation_depth: BlockNumber,
    erasure_coding: ErasureCoding,
) -> Result<InitializedArchiver, SegmentArchiverTaskError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block>,
{
    let best_block_header = chain_info.best_header();
    let best_block_root = *best_block_header.header().root();
    let best_block_number: BlockNumber = best_block_header.header().prefix.number;

    let mut best_block_to_archive = best_block_number.saturating_sub(block_confirmation_depth);

    if (best_block_to_archive..best_block_number).any(|block_number| {
        chain_info
            .ancestor_header(block_number, &best_block_root)
            .is_none()
    }) {
        // If there are blocks missing headers between best block to archive and best block of the
        // blockchain it means newer block was inserted in some special way and as such is by
        // definition valid, so we can simply assume that is our best block to archive instead
        best_block_to_archive = best_block_number;
    }

    let maybe_last_archived_block =
        find_last_archived_block(chain_info, best_block_to_archive, &best_block_root).await;

    let have_last_segment_header = maybe_last_archived_block.is_some();
    let mut best_archived_block = None::<(BlockRoot, BlockNumber)>;

    let mut archiver =
        if let Some((last_segment_header, last_archived_block)) = maybe_last_archived_block {
            // Continuing from existing initial state
            let last_archived_block_number = last_segment_header.last_archived_block.number;
            info!(
                %last_archived_block_number,
                "Resuming archiver from last archived block",
            );

            let last_archived_block_header = last_archived_block.header().header();
            // Set initial value, this is needed in case only genesis block was archived and there
            // is nothing else available
            best_archived_block.replace((
                *last_archived_block_header.root(),
                last_archived_block_header.prefix.number,
            ));

            let last_archived_block_encoded = encode_block(&last_archived_block);

            Archiver::with_initial_state(
                best_block_header.header().prefix.shard_index,
                erasure_coding,
                last_segment_header,
                &last_archived_block_encoded,
                Vec::new(),
            )?
        } else {
            info!("Starting archiving from genesis");

            Archiver::new(
                best_block_header.header().prefix.shard_index,
                erasure_coding,
            )
        };

    // Process blocks since last fully archived block up to the current head minus K
    {
        let blocks_to_archive_from = archiver
            .last_archived_block_number()
            .map(|n| n + BlockNumber::ONE)
            .unwrap_or_default();
        let blocks_to_archive_to = best_block_number
            .checked_sub(block_confirmation_depth)
            .filter(|&blocks_to_archive_to| blocks_to_archive_to >= blocks_to_archive_from)
            .or({
                if have_last_segment_header {
                    None
                } else {
                    // If not continuation, archive genesis block
                    Some(BlockNumber::ZERO)
                }
            });

        if let Some(blocks_to_archive_to) = blocks_to_archive_to {
            info!(
                "Archiving already produced blocks {}..={}",
                blocks_to_archive_from, blocks_to_archive_to,
            );

            for block_number_to_archive in blocks_to_archive_from..=blocks_to_archive_to {
                let header = chain_info
                    .ancestor_header(block_number_to_archive, &best_block_root)
                    .expect("All blocks since last archived must be present; qed");

                let block = chain_info
                    .block(&header.header().root())
                    .await
                    .expect("All blocks since last archived must be present; qed");

                let encoded_block = encode_block(&block);

                debug!(
                    "Encoded block {} has size of {}",
                    block_number_to_archive,
                    ByteSize::b(encoded_block.len() as u64).display().iec(),
                );

                let block_outcome = archiver
                    .add_block(encoded_block, Vec::new())
                    .expect("Block is never empty and doesn't exceed u32; qed");
                let new_segment_headers: Vec<SegmentHeader> = block_outcome
                    .archived_segments
                    .iter()
                    .map(|archived_segment| archived_segment.segment_header)
                    .collect();

                if !new_segment_headers.is_empty() {
                    chain_info
                        .persist_segment_headers(new_segment_headers)
                        .await?;
                }

                if block_number_to_archive == blocks_to_archive_to {
                    best_archived_block.replace((*header.header().root(), block_number_to_archive));
                }
            }
        }
    }

    Ok(InitializedArchiver {
        archiver,
        best_archived_block: best_archived_block
            .expect("Must always set if there is no logical error; qed"),
    })
}

// TODO: Public API for re-archiving of blocks with ability to produce object mappings
/// Create a segment archiver task.
///
/// Archiver task will listen for importing blocks and archive blocks at `K` depth, producing pieces
/// and segment headers. It produces local segments initially, which after confirmation by the
/// beacon chain will be updated with the necessary proof and given a global segment index.
///
/// NOTE: Archiver is doing blocking operations and must run in a dedicated task.
///
/// Archiver is only able to move forward and doesn't support reorgs. Upon restart, it will check
/// segments in [`ChainInfo`] and chain history to reconstruct the "current" state it was in before
/// the last shutdown and continue incrementally archiving blockchain history from there.
///
/// Archiving is triggered by block importing notification (`block_importing_notification_receiver`)
/// and tries to archive the block at [`ConsensusConstants::block_confirmation_depth`] depth from
/// the block being imported. Block importing will then wait for archiver to acknowledge processing,
/// which is necessary for ensuring that when the next block is imported, the newly archived segment
/// is already available deterministically.
///
/// Once a new segment is archived, a notification (`archived_segment_notification_sender`) will be
/// sent and archiver will be paused until all receivers have provided an acknowledgement for it (or
/// a very generous timeout has passed).
pub async fn create_segment_archiver_task<Block, CI>(
    chain_info: CI,
    mut block_importing_notification_receiver: mpsc::Receiver<BlockImportingNotification>,
    mut archived_segment_notification_sender: mpsc::Sender<ArchivedSegmentNotification>,
    consensus_constants: ConsensusConstants,
    erasure_coding: ErasureCoding,
) -> Result<
    impl Future<Output = Result<(), SegmentArchiverTaskError>> + Send + 'static,
    SegmentArchiverTaskError,
>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block> + 'static,
{
    let maybe_archiver = if chain_info.last_segment_header().is_none() {
        let initialize_archiver_fut = initialize_archiver(
            &chain_info,
            consensus_constants.block_confirmation_depth,
            erasure_coding.clone(),
        );
        Some(initialize_archiver_fut.await?)
    } else {
        None
    };

    Ok(async move {
        let archiver = match maybe_archiver {
            Some(archiver) => archiver,
            None => {
                let initialize_archiver_fut = initialize_archiver(
                    &chain_info,
                    consensus_constants.block_confirmation_depth,
                    erasure_coding.clone(),
                );
                initialize_archiver_fut.await?
            }
        };

        let InitializedArchiver {
            mut archiver,
            best_archived_block,
        } = archiver;
        let (mut best_archived_block_root, mut best_archived_block_number) = best_archived_block;

        while let Some(block_importing_notification) =
            block_importing_notification_receiver.next().await
        {
            let importing_block_number = block_importing_notification.block_number;
            let block_number_to_archive = match importing_block_number
                .checked_sub(consensus_constants.block_confirmation_depth)
            {
                Some(block_number_to_archive) => block_number_to_archive,
                None => {
                    // Too early to archive blocks
                    continue;
                }
            };

            let last_archived_block_number = chain_info
                .last_segment_header()
                .expect("Exists after archiver initialization; qed")
                .last_archived_block
                .number();
            trace!(
                %importing_block_number,
                %block_number_to_archive,
                %best_archived_block_number,
                %last_archived_block_number,
                "Checking if block needs to be skipped"
            );

            // Skip archived blocks
            let skip_last_archived_blocks = last_archived_block_number > block_number_to_archive;
            if best_archived_block_number >= block_number_to_archive || skip_last_archived_blocks {
                // This block was already archived, skip
                debug!(
                    %importing_block_number,
                    %block_number_to_archive,
                    %best_archived_block_number,
                    %last_archived_block_number,
                    "Skipping already archived block",
                );
                continue;
            }

            let best_block_root = chain_info.best_root();

            // In case there was a block gap, re-initialize archiver and continue with the current
            // block number (rather than block number at some depth) to allow for special sync
            // modes where pre-verified blocks are inserted at some point in the future comparing to
            // previously existing blocks
            if best_archived_block_number + BlockNumber::ONE != block_number_to_archive {
                let initialize_archiver_fut = initialize_archiver(
                    &chain_info,
                    consensus_constants.block_confirmation_depth,
                    erasure_coding.clone(),
                );
                InitializedArchiver {
                    archiver,
                    best_archived_block: (best_archived_block_root, best_archived_block_number),
                } = initialize_archiver_fut.await?;

                if best_archived_block_number + BlockNumber::ONE == block_number_to_archive {
                    // As expected, can archive this block
                } else if best_archived_block_number >= block_number_to_archive {
                    // Special sync mode where verified blocks were inserted into the blockchain
                    // directly, archiving of this block will naturally happen later
                    continue;
                } else if chain_info
                    .ancestor_header(importing_block_number - BlockNumber::ONE, &best_block_root)
                    .is_none()
                {
                    // We may have imported some block using special sync mode, so the block about
                    // to be imported is the first one after the gap at which archiver is supposed
                    // to be initialized, but we are only about to import it, so wait for the next
                    // block for now
                    continue;
                } else {
                    return Err(SegmentArchiverTaskError::BlockGap {
                        best_archived_block_number,
                        block_number_to_archive,
                        importing_block_number,
                    });
                }
            }

            (best_archived_block_root, best_archived_block_number) = archive_block(
                &mut archiver,
                &chain_info,
                &mut archived_segment_notification_sender,
                best_archived_block_root,
                block_number_to_archive,
                &best_block_root,
            )
            .await?;
        }

        Ok(())
    })
}

/// Tries to archive `block_number` and returns new (or old if not changed) best archived block
async fn archive_block<Block, CI>(
    archiver: &mut Archiver,
    chain_info: &CI,
    archived_segment_notification_sender: &mut mpsc::Sender<ArchivedSegmentNotification>,
    best_archived_block_root: BlockRoot,
    block_number_to_archive: BlockNumber,
    best_block_root: &BlockRoot,
) -> Result<(BlockRoot, BlockNumber), SegmentArchiverTaskError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block>,
{
    let header = chain_info
        .ancestor_header(block_number_to_archive, best_block_root)
        .expect("All blocks since last archived must be present; qed");

    let parent_block_root = header.header().prefix.parent_root;
    if parent_block_root != best_archived_block_root {
        return Err(SegmentArchiverTaskError::ArchivingReorg {
            parent_block_root,
            best_archived_block_root,
        });
    }

    let block_root_to_archive = *header.header().root();

    let block = chain_info
        .block(&block_root_to_archive)
        .await
        .expect("All blocks since last archived must be present; qed");

    debug!("Archiving block {block_number_to_archive} ({block_root_to_archive})");

    let encoded_block = encode_block(&block);
    debug!(
        "Encoded block {block_number_to_archive} has size of {}",
        ByteSize::b(encoded_block.len() as u64).display().iec(),
    );

    let block_outcome = archiver
        .add_block(encoded_block, Vec::new())
        .expect("Block is never empty and doesn't exceed u32; qed");
    for archived_segment in block_outcome.archived_segments {
        let segment_header = archived_segment.segment_header;

        chain_info
            .persist_segment_headers(vec![segment_header])
            .await?;

        send_archived_segment_notification(archived_segment_notification_sender, archived_segment)
            .await;
    }

    Ok((block_root_to_archive, block_number_to_archive))
}

async fn send_archived_segment_notification(
    archived_segment_notification_sender: &mut mpsc::Sender<ArchivedSegmentNotification>,
    archived_segment: NewArchivedSegment,
) {
    let segment_index = archived_segment.segment_header.segment_index;
    let (acknowledgement_sender, mut acknowledgement_receiver) = mpsc::channel(1);
    // Keep `archived_segment` around until all acknowledgements are received since some receivers
    // might use weak references
    let archived_segment = Arc::new(archived_segment);
    let archived_segment_notification = ArchivedSegmentNotification {
        archived_segment: Arc::clone(&archived_segment),
        acknowledgement_sender,
    };

    if let Err(error) = archived_segment_notification_sender
        .send(archived_segment_notification)
        .await
    {
        warn!(
            %error,
            "Failed to send archived segment notification"
        );
    }

    let wait_fut = async {
        while acknowledgement_receiver.next().await.is_some() {
            debug!(
                "Archived segment notification acknowledged: {}",
                segment_index
            );
        }
    };

    if tokio::time::timeout(ACKNOWLEDGEMENT_TIMEOUT, wait_fut)
        .await
        .is_err()
    {
        warn!(
            "Archived segment notification was not acknowledged and reached timeout, continue \
            regardless"
        );
    }
}
