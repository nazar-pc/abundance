use crate::task::{
    ArchivedSegmentNotification, SegmentArchiverTaskError, create_segment_archiver_task,
};
use ab_client_api::{
    BlockDetails, ChainInfo, ChainInfoWrite, PersistBlockError, PersistSegmentHeadersError,
    ReadBlockError,
};
use ab_client_consensus_common::{
    BlockImportingNotification, ConsensusConstants, PotConsensusConstants,
};
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderSeal, GenericBlockHeader,
};
use ab_core_primitives::block::owned::{
    GenericOwnedBlock, OwnedBeaconChainBlock, OwnedBeaconChainBlockBuilder,
};
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotCheckpoints, PotOutput, SlotDuration, SlotNumber};
use ab_core_primitives::segments::{HistorySize, LocalSegmentIndex, SegmentHeader};
use ab_core_primitives::shard::{NumShards, ShardIndex};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use ab_erasure_coding::ErasureCoding;
use futures::channel::mpsc;
use futures::prelude::*;
use std::collections::HashMap;
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use std::sync::Mutex;
use std::{future, panic};
use tokio::task::JoinHandle;
use triomphe::Arc;

/// Builds beacon chain blocks that all share the same body
#[derive(Debug)]
pub(crate) struct BeaconChainBlockBuilder {
    body: OwnedBeaconChainBlockBuilder,
}

impl BeaconChainBlockBuilder {
    /// Blocks with a body of about `body_size` bytes, made of PoT checkpoints
    pub(crate) fn new(body_size: usize) -> Self {
        let pot_checkpoints = vec![PotCheckpoints::default(); body_size / PotCheckpoints::SIZE];

        Self {
            body: OwnedBeaconChainBlock::init([].into_iter(), [].into_iter(), &pot_checkpoints)
                .unwrap(),
        }
    }

    pub(crate) fn genesis(&self) -> OwnedBeaconChainBlock {
        self.block(BlockNumber::ZERO, BlockRoot::default())
    }

    pub(crate) fn child(&self, parent: &OwnedBeaconChainBlock) -> OwnedBeaconChainBlock {
        let parent_header = parent.header.header();

        self.block(
            parent_header.prefix.number + BlockNumber::ONE,
            *parent_header.root(),
        )
    }

    /// Block `number` with `parent_root`, the parent doesn't have to exist
    pub(crate) fn block(
        &self,
        number: BlockNumber,
        parent_root: BlockRoot,
    ) -> OwnedBeaconChainBlock {
        self.body
            .clone()
            .with_header(
                &BlockHeaderPrefix {
                    number,
                    shard_index: ShardIndex::BEACON_CHAIN,
                    padding_0: [0; _],
                    timestamp: BlockTimestamp::from_millis(0),
                    parent_root,
                    mmr_root: Blake3Hash::default(),
                },
                Blake3Hash::default(),
                &BlockHeaderConsensusInfo {
                    slot: SlotNumber::ZERO,
                    proof_of_time: PotOutput::default(),
                    future_proof_of_time: PotOutput::default(),
                    solution: Solution::genesis_solution(),
                },
                &BlockHeaderConsensusParameters {
                    fixed_parameters: BlockHeaderFixedConsensusParameters {
                        solution_range: SolutionRange::from_pieces(1000, (1, 10)),
                        slot_iterations: NonZeroU32::new(256).unwrap(),
                        num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
                    },
                    super_segment_root: None,
                    next_solution_range: None,
                    pot_parameters_change: None,
                },
            )
            .with_seal(BlockHeaderSeal::Ed25519(&BlockHeaderEd25519Seal {
                public_key: Ed25519PublicKey::default(),
                signature: Ed25519Signature::default(),
            }))
    }
}

#[derive(Debug)]
struct MockChainState<Block> {
    blocks: HashMap<BlockRoot, Block>,
    best_root: BlockRoot,
    segment_headers: Vec<SegmentHeader>,
    read_block_numbers: Vec<BlockNumber>,
}

impl<Block> MockChainState<Block> {
    /// Like the database, skip segment headers that are already stored and reject those that don't
    /// follow the last one
    fn add_segment_headers(
        &mut self,
        segment_headers: Vec<SegmentHeader>,
    ) -> Result<(), PersistSegmentHeadersError> {
        for segment_header in segment_headers {
            let local_segment_index = segment_header.index.as_inner();

            if let Some(last_segment_header) = self.segment_headers.last() {
                let last_local_segment_index = last_segment_header.index.as_inner();

                if local_segment_index <= last_local_segment_index {
                    continue;
                }
                if local_segment_index != last_local_segment_index + LocalSegmentIndex::ONE {
                    return Err(PersistSegmentHeadersError::MustFollowLastSegmentIndex {
                        local_segment_index,
                        last_local_segment_index,
                    });
                }
            } else if local_segment_index != LocalSegmentIndex::ZERO {
                return Err(PersistSegmentHeadersError::FirstSegmentIndexZero {
                    local_segment_index,
                });
            }

            self.segment_headers.push(segment_header);
        }

        Ok(())
    }
}

/// Mock of [`ChainInfoWrite`] with a tree of blocks and segment headers, clones share the same
/// state like clones of the database do.
///
/// The block with the highest number is the best block. Unlike the database, blocks are added
/// without checking that their parents are known and without removing other blocks, which lets
/// tests insert blocks the way Snap sync does. Methods that the archiver doesn't use are not
/// implemented.
#[derive(Debug, Clone)]
pub(crate) struct MockChain<Block> {
    state: Arc<Mutex<MockChainState<Block>>>,
}

impl<Block> ChainInfo<Block> for MockChain<Block>
where
    Block: GenericOwnedBlock,
{
    fn best_root(&self) -> BlockRoot {
        self.state.lock().unwrap().best_root
    }

    fn best_header(&self) -> Block::Header {
        let state = self.state.lock().unwrap();

        state
            .blocks
            .get(&state.best_root)
            .expect("Best block exists")
            .header()
            .clone()
    }

    fn best_header_with_details(&self) -> (Block::Header, BlockDetails) {
        unimplemented!()
    }

    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<Block::Header> {
        let state = self.state.lock().unwrap();

        let mut block = state.blocks.get(descendant_block_root)?;
        while block.header().header().prefix.number > ancestor_block_number {
            block = state
                .blocks
                .get(&block.header().header().prefix.parent_root)?;
        }

        (block.header().header().prefix.number == ancestor_block_number)
            .then(|| block.header().clone())
    }

    fn header(&self, _block_root: &BlockRoot) -> Option<Block::Header> {
        unimplemented!()
    }

    fn header_with_details(
        &self,
        _block_root: &BlockRoot,
    ) -> Option<(Block::Header, BlockDetails)> {
        unimplemented!()
    }

    fn block(
        &self,
        block_root: &BlockRoot,
    ) -> impl Future<Output = Result<Block, ReadBlockError>> + Send {
        let mut state = self.state.lock().unwrap();

        let maybe_block = state.blocks.get(block_root).cloned();
        if let Some(block) = &maybe_block {
            state
                .read_block_numbers
                .push(block.header().header().prefix.number);
        }

        future::ready(maybe_block.ok_or(ReadBlockError::UnknownBlockRoot))
    }

    fn last_segment_header(&self) -> Option<SegmentHeader> {
        self.state.lock().unwrap().segment_headers.last().copied()
    }

    fn get_segment_header(&self, segment_index: LocalSegmentIndex) -> Option<SegmentHeader> {
        self.state
            .lock()
            .unwrap()
            .segment_headers
            .get(usize::try_from(u64::from(segment_index)).unwrap())
            .copied()
    }

    fn segment_headers_for_block(&self, _block_number: BlockNumber) -> Vec<SegmentHeader> {
        unimplemented!()
    }
}

impl<Block> ChainInfoWrite<Block> for MockChain<Block>
where
    Block: GenericOwnedBlock,
{
    async fn persist_block(
        &self,
        _block: Block,
        _block_details: BlockDetails,
    ) -> Result<(), PersistBlockError> {
        unimplemented!()
    }

    fn persist_segment_headers(
        &self,
        segment_headers: Vec<SegmentHeader>,
    ) -> impl Future<Output = Result<(), PersistSegmentHeadersError>> + Send {
        future::ready(
            self.state
                .lock()
                .unwrap()
                .add_segment_headers(segment_headers),
        )
    }
}

impl<Block> MockChain<Block>
where
    Block: GenericOwnedBlock,
{
    /// Chain with just `first_block`, which is the genesis block or a block that Snap sync
    /// inserted directly
    pub(crate) fn new(first_block: Block) -> Self {
        let first_block_root = *first_block.header().header().root();

        Self {
            state: Arc::new(Mutex::new(MockChainState {
                blocks: HashMap::from([(first_block_root, first_block)]),
                best_root: first_block_root,
                segment_headers: Vec::new(),
                read_block_numbers: Vec::new(),
            })),
        }
    }

    /// Add `block`, which becomes the best block if its number is higher than that of the best
    /// block
    pub(crate) fn add_block(&self, block: Block) {
        let mut state = self.state.lock().unwrap();
        let block_root = *block.header().header().root();

        if block.header().header().prefix.number
            > state
                .blocks
                .get(&state.best_root)
                .expect("Best block exists")
                .header()
                .header()
                .prefix
                .number
        {
            state.best_root = block_root;
        }
        state.blocks.insert(block_root, block);
    }

    pub(crate) fn segment_headers(&self) -> Vec<SegmentHeader> {
        self.state.lock().unwrap().segment_headers.clone()
    }

    /// Numbers of blocks that were read with [`ChainInfo::block()`] so far, which is how the
    /// archiver reads blocks to archive
    pub(crate) fn read_block_numbers(&self) -> Vec<BlockNumber> {
        self.state.lock().unwrap().read_block_numbers.clone()
    }
}

/// Consensus constants with `block_confirmation_depth`, the rest doesn't matter to the archiver
pub(crate) fn consensus_constants(block_confirmation_depth: BlockNumber) -> ConsensusConstants {
    ConsensusConstants {
        block_confirmation_depth,
        shard_confirmation_depth: BlockNumber::from(2),
        block_authoring_delay: SlotNumber::from(4),
        pot: PotConsensusConstants {
            entropy_injection_interval: BlockNumber::from(50),
            entropy_injection_lookback_depth: 2,
            entropy_injection_delay: SlotNumber::from(15),
        },
        retarget_interval: BlockNumber::from(180),
        slot_probability: (1, 10),
        slot_duration: SlotDuration::from_millis(1000),
        recent_segments: HistorySize::new(NonZeroU64::new(5).unwrap()),
        recent_history_fraction: (
            HistorySize::new(NonZeroU64::new(1).unwrap()),
            HistorySize::new(NonZeroU64::new(10).unwrap()),
        ),
        min_sector_lifetime: HistorySize::new(NonZeroU64::new(4).unwrap()),
        max_block_timestamp_drift: BlockTimestamp::from_millis(30_000),
        shard_rotation_interval: BlockNumber::from(36),
        shard_rotation_delay: BlockNumber::from(18),
    }
}

/// Run `fut` on a runtime that the archiver task can run on
pub(crate) fn run<Fut>(fut: Fut) -> Fut::Output
where
    Fut: Future,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(fut)
}

/// Segment archiver task on a [`MockChain`], driven the same way `BeaconChainBlockImport` does it
#[derive(Debug)]
pub(crate) struct ArchiverTask<Block> {
    chain: MockChain<Block>,
    block_importing_notification_sender: mpsc::Sender<BlockImportingNotification>,
    task: JoinHandle<Result<(), SegmentArchiverTaskError>>,
    archived_segment_indices: Arc<Mutex<Vec<LocalSegmentIndex>>>,
}

impl<Block> ArchiverTask<Block>
where
    Block: GenericOwnedBlock,
{
    /// Create the task and spawn it, archived segment notifications are acknowledged right away
    pub(crate) async fn start(
        chain: &MockChain<Block>,
        block_confirmation_depth: BlockNumber,
    ) -> Result<Self, SegmentArchiverTaskError> {
        let (block_importing_notification_sender, block_importing_notification_receiver) =
            mpsc::channel(0);
        let (archived_segment_notification_sender, mut archived_segment_notification_receiver) =
            mpsc::channel::<ArchivedSegmentNotification>(0);

        let task = create_segment_archiver_task(
            chain.clone(),
            block_importing_notification_receiver,
            archived_segment_notification_sender,
            consensus_constants(block_confirmation_depth),
            ErasureCoding::new(),
        )
        .await?;

        let archived_segment_indices = Arc::<Mutex<Vec<LocalSegmentIndex>>>::default();
        tokio::spawn({
            let archived_segment_indices = Arc::clone(&archived_segment_indices);

            async move {
                while let Some(notification) = archived_segment_notification_receiver.next().await {
                    archived_segment_indices.lock().unwrap().push(
                        notification
                            .archived_segment
                            .segment_header
                            .index
                            .as_inner(),
                    );
                }
            }
        });

        Ok(Self {
            chain: chain.clone(),
            block_importing_notification_sender,
            task: tokio::spawn(task),
            archived_segment_indices,
        })
    }

    /// Import `block` like `BeaconChainBlockImport::import()` does: send the block importing
    /// notification, wait for it to be acknowledged, then add the block to the chain
    pub(crate) async fn import(&mut self, block: Block) {
        let (acknowledgement_sender, mut acknowledgement_receiver) = mpsc::channel(0);
        // The task may have exited with an error, which `Self::stop()` returns
        let _: Result<(), mpsc::SendError> = self
            .block_importing_notification_sender
            .send(BlockImportingNotification {
                block_number: block.header().header().prefix.number,
                acknowledgement_sender,
            })
            .await;
        while acknowledgement_receiver.next().await.is_some() {
            // Wait for all the acknowledgements to arrive
        }

        self.chain.add_block(block);
    }

    /// Indices of segments in archived segment notifications so far
    pub(crate) fn archived_segment_indices(&self) -> Vec<LocalSegmentIndex> {
        self.archived_segment_indices.lock().unwrap().clone()
    }

    /// Stop the task like block import shutdown does and return its result, a panic in the task
    /// is resumed
    pub(crate) async fn stop(self) -> Result<(), SegmentArchiverTaskError> {
        let Self {
            chain: _,
            block_importing_notification_sender,
            task,
            archived_segment_indices: _,
        } = self;
        drop(block_importing_notification_sender);

        task.await
            .unwrap_or_else(|error| panic::resume_unwind(error.into_panic()))
    }
}
