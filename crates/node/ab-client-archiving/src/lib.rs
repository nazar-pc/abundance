//! Consensus archiver responsible for archival of blockchain history, it is driven by the block
//! import pipeline.
//!
//! Implements archiving process in that converts blockchain history (blocks) into archived history
//! (segments and pieces).
//!
//! The main entry point here is [`create_archiver_task`] that will create a task, which while
//! driven will perform the archiving itself.
//!
//! Archiving is triggered by block importing notification and tries to archive the block at
//! [`ConsensusConstants::confirmation_depth_k`](ab_client_consensus_common::ConsensusConstants::confirmation_depth_k)
//! depth from the block being imported. Block import will then wait for archiver to acknowledge
//! processing, which is necessary for ensuring that when the next block is imported, it will
//! contain a segment header of the newly archived block (must happen exactly in the next block).
//!
//! Archiving itself will also wait for acknowledgement by various subscribers before proceeding,
//! which includes farmer subscription, in case of reference implementation via RPC.
//!
//! Known segment headers contain all known (including future in case of syncing) segment headers.
//! It is available to other parts of the protocol that need to know what the correct archival
//! history of the blockchain looks like through [`ChainInfo`]. For example, it is used during node
//! sync and farmer plotting to verify pieces of archival history received from other network
//! participants.
//!
//! [`recreate_genesis_segment`] is a bit of a hack and is useful for deriving of the genesis
//! segment that is a special case since we don't have enough data in the blockchain history itself
//! during genesis to do the archiving.
//!
//! [`encode_block`] and [`decode_block`] are symmetric encoding/decoding functions turning
//! Blocks into bytes and back.

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_archiving::archiver::{Archiver, ArchiverInstantiationError, NewArchivedSegment};
use ab_archiving::objects::{BlockObject, GlobalObject};
use ab_client_api::{ChainInfo, ChainInfoWrite, PersistSegmentHeadersError};
use ab_client_consensus_common::{BlockImportingNotification, ConsensusConstants};
use ab_core_primitives::block::body::owned::GenericOwnedBlockBody;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBeaconChainBlock};
use ab_core_primitives::block::{BlockNumber, BlockRoot, GenericBlock};
use ab_core_primitives::segments::{RecordedHistorySegment, SegmentHeader, SegmentIndex};
use ab_core_primitives::shard::RealShardKind;
use ab_erasure_coding::ErasureCoding;
use bytesize::ByteSize;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use futures::channel::mpsc;
use futures::prelude::*;
use std::num::NonZeroU64;
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
// const FINALIZATION_DEPTH_IN_SEGMENTS: SegmentIndex = SegmentIndex::new(5);

/// Notification with a new archived segment that was just archived
#[derive(Debug)]
pub struct ArchivedSegmentNotification {
    /// Archived segment.
    pub archived_segment: Arc<NewArchivedSegment>,
    /// Sender that signified the fact of receiving archived segment by farmer.
    ///
    /// This must be used to send a message or else block import pipeline will get stuck.
    pub acknowledgement_sender: mpsc::Sender<()>,
}

/// Notification with incrementally generated object mappings for a block (and any previous block
/// continuation)
#[derive(Debug, Clone)]
pub struct ObjectMappingNotification {
    /// Incremental object mappings for a block (and any previous block continuation).
    ///
    /// The archived data won't be available in pieces until the entire segment is full and
    /// archived.
    pub object_mapping: Vec<GlobalObject>,
    /// The block that these mappings are from.
    pub block_number: BlockNumber,
    // TODO: add an acknowledgement_sender for backpressure if needed
}

/// Whether to create object mappings.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum CreateObjectMappings {
    /// Start creating object mappings from this block number.
    ///
    /// This can be lower than the latest archived block, but must be greater than genesis.
    ///
    /// The genesis block doesn't have mappings, so starting mappings at genesis is pointless.
    /// The archiver will fail if it can't get the data for this block, but snap sync doesn't store
    /// the genesis data on disk.  So avoiding genesis also avoids this error.
    /// <https://github.com/paritytech/polkadot-sdk/issues/5366>
    Block(NonZeroU64),
    /// Create object mappings as archiving is happening
    Yes,
    /// Don't create object mappings
    #[default]
    No,
}

impl CreateObjectMappings {
    /// The fixed block number to start creating object mappings from.
    /// If there is no fixed block number, or mappings are disabled, returns None.
    fn block(&self) -> Option<BlockNumber> {
        match self {
            CreateObjectMappings::Block(block) => Some(BlockNumber::new(block.get())),
            CreateObjectMappings::Yes => None,
            CreateObjectMappings::No => None,
        }
    }

    /// Returns true if object mappings will be created from a past or future block.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CreateObjectMappings::No)
    }

    /// Does the supplied block number need object mappings?
    pub fn is_enabled_for_block(&self, block: BlockNumber) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if let Some(target_block) = self.block() {
            return block >= target_block;
        }

        // We're continuing where we left off, so all blocks get mappings.
        true
    }
}

async fn find_last_archived_block<Block, CI, COM>(
    chain_info: &CI,
    best_block_number_to_archive: BlockNumber,
    best_block_root: &BlockRoot,
    create_object_mappings: Option<COM>,
) -> Option<(SegmentHeader, Block, Vec<BlockObject>)>
where
    Block: GenericOwnedBlock,
    CI: ChainInfo<Block>,
    COM: Fn(&Block) -> Vec<BlockObject>,
{
    let max_segment_index = chain_info.max_segment_index()?;

    if max_segment_index == SegmentIndex::ZERO {
        // Just genesis, nothing else to check
        return None;
    }

    for segment_header in (SegmentIndex::ZERO..=max_segment_index)
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

        // If we're starting mapping creation at this block, return its mappings
        let block_object_mappings = if let Some(create_object_mappings) = create_object_mappings {
            create_object_mappings(&last_archived_block)
        } else {
            Vec::new()
        };

        return Some((segment_header, last_archived_block, block_object_mappings));
    }

    None
}

/// Derive the genesis segment on demand, returns `Ok(None)` in case the genesis block was already
/// pruned
pub fn recreate_genesis_segment(
    owned_genesis_block: &OwnedBeaconChainBlock,
    erasure_coding: ErasureCoding,
) -> NewArchivedSegment {
    let encoded_block = encode_block(owned_genesis_block);

    // There are no mappings in the genesis block, so they can be ignored
    let block_outcome = Archiver::new(erasure_coding)
        .add_block(encoded_block, Vec::new())
        .expect("Block is never empty and doesn't exceed u32; qed");
    block_outcome
        .archived_segments
        .into_iter()
        .next()
        .expect("Genesis block always results in exactly one archived segment; qed")
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

/// Archiver task error
#[derive(Debug, thiserror::Error)]
pub enum ArchiverTaskError {
    /// Archiver instantiation error
    #[error("Archiver instantiation error: {error}")]
    Instantiation {
        /// Low-level error
        #[from]
        error: ArchiverInstantiationError,
    },
    /// Failed to persis a new segment header
    #[error("Failed to persis a new segment header: {error}")]
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
    confirmation_depth_k: BlockNumber,
    create_object_mappings: CreateObjectMappings,
    erasure_coding: ErasureCoding,
) -> Result<InitializedArchiver, ArchiverTaskError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block>,
{
    let best_block_header = chain_info.best_header();
    let best_block_root = *best_block_header.header().root();
    let best_block_number: BlockNumber = best_block_header.header().prefix.number;

    let mut best_block_to_archive = best_block_number.saturating_sub(confirmation_depth_k);
    // Choose a lower block number if we want to get mappings from that specific block.
    // If we are continuing from where we left off, we don't need to change the block number to
    // archive. If there is no path to this block from the tip due to snap sync, we'll start
    // archiving from an earlier segment, then start mapping again once archiving reaches this
    // block.
    if let Some(block_number) = create_object_mappings.block() {
        // There aren't any mappings in the genesis block, so starting there is pointless.
        // (And causes errors on restart, because genesis block data is never stored during snap
        // sync.)
        best_block_to_archive = best_block_to_archive.min(block_number);
    }

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

    // TODO: Uncomment once API for object mapping is established
    // // If the user chooses an object mapping start block we don't have data or state for, we
    // // can't create mappings for it, so the node must exit with an error. We ignore genesis
    // // here, because it doesn't have mappings.
    // if create_object_mappings.is_enabled() && best_block_to_archive >= BlockNumber::ONE {
    //     let Some(best_block_to_archive_root) = client.root(NumberFor::<Block>::saturated_from(
    //         best_block_to_archive.as_u64(),
    //     ))?
    //     else {
    //         let error = format!(
    //             "Missing root for mapping block {best_block_to_archive}, \
    //             try a higher block number, or wipe your node and restart with `--sync full`"
    //         );
    //         return Err(sp_blockchain::Error::Application(error.into()));
    //     };
    //
    //     let Some(_best_block_data) = client.block(best_block_to_archive_root)? else {
    //         let error = format!(
    //             "Missing data for mapping block {best_block_to_archive} \
    //             root {best_block_to_archive_root}, \
    //             try a higher block number, or wipe your node and restart with `--sync full`"
    //         );
    //         return Err(sp_blockchain::Error::Application(error.into()));
    //     };
    //
    //     // Similarly, state can be pruned, even if the data is present
    //     // TODO: Injection of external logic
    //     // client
    //     //     .runtime_api()
    //     //     .extract_block_object_mapping(
    //     //         *best_block_data.block.header().parent_root(),
    //     //         best_block_data.block.clone(),
    //     //     )
    //     //     .map_err(|error| {
    //     //         sp_blockchain::Error::Application(
    //     //             format!(
    //     //                 "Missing state for mapping block {best_block_to_archive} \
    //     //                 root {best_block_to_archive_root}: {error}, \
    //     //                 try a higher block number, or wipe your node and restart with
    //     //                 `--sync full`"
    //     //             )
    //     //             .into(),
    //     //         )
    //     //     })?;
    // }

    let maybe_last_archived_block = find_last_archived_block(
        chain_info,
        best_block_to_archive,
        &best_block_root,
        create_object_mappings
            .is_enabled()
            .then_some(|_block: &Block| {
                // TODO: Injection of external logic
                // let parent_root = *block.header().parent_root();
                // client
                //     .runtime_api()
                //     .extract_block_object_mapping(parent_root, block)
                //     .unwrap_or_default()
                Vec::new()
            }),
    )
    .await;

    let have_last_segment_header = maybe_last_archived_block.is_some();
    let mut best_archived_block = None::<(BlockRoot, BlockNumber)>;

    let mut archiver =
        if let Some((last_segment_header, last_archived_block, block_object_mappings)) =
            maybe_last_archived_block
        {
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
                erasure_coding,
                last_segment_header,
                &last_archived_block_encoded,
                block_object_mappings,
            )?
        } else {
            info!("Starting archiving from genesis");

            Archiver::new(erasure_coding)
        };

    // Process blocks since last fully archived block up to the current head minus K
    {
        let blocks_to_archive_from = archiver
            .last_archived_block_number()
            .map(|n| n + BlockNumber::ONE)
            .unwrap_or_default();
        let blocks_to_archive_to = best_block_number
            .checked_sub(confirmation_depth_k)
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

                let block_object_mappings =
                    if create_object_mappings.is_enabled_for_block(block_number_to_archive) {
                        // TODO: Injection of external logic
                        // runtime_api
                        //     .extract_block_object_mapping(
                        //         *block.block.header().parent_root(),
                        //         block.block.clone(),
                        //     )
                        //     .unwrap_or_default()
                        Vec::new()
                    } else {
                        Vec::new()
                    };

                let encoded_block = encode_block(&block);

                debug!(
                    "Encoded block {} has size of {}",
                    block_number_to_archive,
                    ByteSize::b(encoded_block.len() as u64).display().iec(),
                );

                let block_outcome = archiver
                    .add_block(encoded_block, block_object_mappings)
                    .expect("Block is never empty and doesn't exceed u32; qed");
                // TODO: Allow to capture these from the outside
                // send_object_mapping_notification(
                //     &subspace_link.object_mapping_notification_sender,
                //     block_outcome.global_objects,
                //     block_number_to_archive,
                // );
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

/// Create an archiver task.
///
/// Archiver task will listen for importing blocks and archive blocks at `K` depth, producing pieces
/// and segment headers (segment headers are then added back to the blockchain in the next block).
///
/// NOTE: Archiver is doing blocking operations and must run in a dedicated task.
///
/// Archiver is only able to move forward and doesn't support reorgs. Upon restart, it will check
/// segments in [`ChainInfo`] and chain history to reconstruct the "current" state it was in before
/// the last shutdown and continue incrementally archiving blockchain history from there.
///
/// Archiving is triggered by block importing notification (`block_importing_notification_receiver`)
/// and tries to archive the block at [`ConsensusConstants::confirmation_depth_k`] depth from the
/// block being imported. Block importing will then wait for archiver to acknowledge processing,
/// which is necessary for ensuring that when the next block is imported, the body will contain a
/// segment header of the newly archived segment.
///
/// `create_object_mappings` controls when object mappings are created for archived blocks. When
/// these mappings are created.
///
/// Once a new segment is archived, a notification (`archived_segment_notification_sender`) will be
/// sent and archiver will be paused until all receivers have provided an acknowledgement for it.
pub async fn create_archiver_task<Block, CI>(
    chain_info: CI,
    mut block_importing_notification_receiver: mpsc::Receiver<BlockImportingNotification>,
    mut archived_segment_notification_sender: mpsc::Sender<ArchivedSegmentNotification>,
    consensus_constants: ConsensusConstants,
    create_object_mappings: CreateObjectMappings,
    erasure_coding: ErasureCoding,
) -> Result<impl Future<Output = Result<(), ArchiverTaskError>> + Send + 'static, ArchiverTaskError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block> + 'static,
{
    if create_object_mappings.is_enabled() {
        info!(
            ?create_object_mappings,
            "Creating object mappings from the configured block onwards"
        );
    } else {
        info!("Not creating object mappings");
    }

    let maybe_archiver = if chain_info.max_segment_index().is_none() {
        let initialize_archiver_fut = initialize_archiver(
            &chain_info,
            consensus_constants.confirmation_depth_k,
            create_object_mappings,
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
                    consensus_constants.confirmation_depth_k,
                    create_object_mappings,
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
                .checked_sub(consensus_constants.confirmation_depth_k)
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
            let create_mappings =
                create_object_mappings.is_enabled_for_block(last_archived_block_number);
            trace!(
                %importing_block_number,
                %block_number_to_archive,
                %best_archived_block_number,
                %last_archived_block_number,
                "Checking if block needs to be skipped"
            );

            // Skip archived blocks, unless we're producing object mappings for them
            let skip_last_archived_blocks =
                last_archived_block_number > block_number_to_archive && !create_mappings;
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
                    consensus_constants.confirmation_depth_k,
                    create_object_mappings,
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
                    return Err(ArchiverTaskError::BlockGap {
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
                create_object_mappings,
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
    // TODO: Probably remove
    // object_mapping_notification_sender: SubspaceNotificationSender<ObjectMappingNotification>,
    archived_segment_notification_sender: &mut mpsc::Sender<ArchivedSegmentNotification>,
    best_archived_block_root: BlockRoot,
    block_number_to_archive: BlockNumber,
    best_block_root: &BlockRoot,
    create_object_mappings: CreateObjectMappings,
) -> Result<(BlockRoot, BlockNumber), ArchiverTaskError>
where
    Block: GenericOwnedBlock,
    CI: ChainInfoWrite<Block>,
{
    let header = chain_info
        .ancestor_header(block_number_to_archive, best_block_root)
        .expect("All blocks since last archived must be present; qed");

    let parent_block_root = header.header().prefix.parent_root;
    if parent_block_root != best_archived_block_root {
        return Err(ArchiverTaskError::ArchivingReorg {
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

    let create_mappings = create_object_mappings.is_enabled_for_block(block_number_to_archive);

    let block_object_mappings = if create_mappings {
        // TODO: Injection of external logic
        // client
        //     .runtime_api()
        //     .extract_block_object_mapping(parent_block_root, block.block.clone())
        //     .map_err(|error| {
        //         sp_blockchain::Error::Application(
        //             format!("Failed to retrieve block object mappings: {error}").into(),
        //         )
        //     })?
        Vec::new()
    } else {
        Vec::new()
    };

    let encoded_block = encode_block(&block);
    debug!(
        "Encoded block {block_number_to_archive} has size of {}",
        ByteSize::b(encoded_block.len() as u64).display().iec(),
    );

    let block_outcome = archiver
        .add_block(encoded_block, block_object_mappings)
        .expect("Block is never empty and doesn't exceed u32; qed");
    // TODO: Probably remove
    // send_object_mapping_notification(
    //     &object_mapping_notification_sender,
    //     block_outcome.global_objects,
    //     block_number_to_archive,
    // );
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

// TODO: Probably remove
// fn send_object_mapping_notification(
//     object_mapping_notification_sender: &SubspaceNotificationSender<ObjectMappingNotification>,
//     object_mapping: Vec<GlobalObject>,
//     block_number: BlockNumber,
// ) {
//     if object_mapping.is_empty() {
//         return;
//     }
//
//     let object_mapping_notification = ObjectMappingNotification {
//         object_mapping,
//         block_number,
//     };
//
//     object_mapping_notification_sender.notify(move || object_mapping_notification);
// }

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
