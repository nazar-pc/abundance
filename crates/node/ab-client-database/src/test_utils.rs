use crate::storage_backend::{
    ClientDatabaseStorageBackend, OwnedPages, SharedPages, StorageBackendError,
};
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::{
    ClientDatabase, ClientDatabaseError, ClientDatabaseFormatOptions, ClientDatabaseInnerOptions,
    ClientDatabaseOptions, GenesisBlockBuilderResult,
};
use ab_client_api::{BlockDetails, ChainInfoWrite, PersistBlockError};
use ab_core_primitives::block::body::owned::{
    OwnedIntermediateShardBlockInfo, OwnedLeafShardBlockInfo,
};
use ab_core_primitives::block::header::{
    BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderEd25519Seal, BlockHeaderFixedConsensusParameters, BlockHeaderPrefix,
    BlockHeaderSeal,
};
use ab_core_primitives::block::owned::{
    OwnedBeaconChainBlock, OwnedBeaconChainBlockBuilder, OwnedIntermediateShardBlock,
    OwnedLeafShardBlock,
};
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotOutput, SlotNumber};
use ab_core_primitives::segments::{
    ArchivedBlockProgress, LastArchivedBlock, LocalSegmentIndex, SegmentHeader, SegmentIndex,
    SegmentRoot, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_core_primitives::shard::{NumShards, ShardIndex};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use ab_core_primitives::trees::BlockMerkleMountainRange;
use futures::channel::oneshot;
use futures::executor::block_on;
use std::mem::MaybeUninit;
use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};
use std::ops::RangeInclusive;
use std::sync::Mutex;
use std::{fmt, io, slice};
use triomphe::Arc;

const NUM_PAGES: u32 = 4096;
pub(crate) const PAGE_GROUP_SIZE: NonZeroU32 = NonZeroU32::new(256).unwrap();

/// Write that is held back by [`MemoryStorageBackend::hold_writes()`]
struct HeldWrite {
    buffer: SharedPages,
    offset: u32,
    sender: oneshot::Sender<(Result<(), StorageBackendError>, SharedPages)>,
}

/// In-memory storage backend, clones share the same pages, so a database can be reopened on them
/// after the previous instance was dropped
#[derive(Clone)]
pub(crate) struct MemoryStorageBackend {
    bytes: Arc<Mutex<Vec<u8>>>,
    /// Number of writes that still reach the pages, `None` means unlimited
    remaining_writes: Arc<Mutex<Option<usize>>>,
    /// Writes that neither reached the pages nor finished yet, `None` means writes are not held
    held_writes: Arc<Mutex<Option<Vec<HeldWrite>>>>,
}

impl fmt::Debug for MemoryStorageBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStorageBackend")
            .finish_non_exhaustive()
    }
}

impl ClientDatabaseStorageBackend for MemoryStorageBackend {
    fn num_pages(&self) -> u32 {
        u32::try_from(self.bytes.lock().unwrap().len()).unwrap() / OwnedPages::ALIGN
    }

    fn read(
        &self,
        mut buffer: OwnedPages,
        num_pages: u32,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, OwnedPages)> {
        let (sender, receiver) = oneshot::channel();
        let offset = (offset * OwnedPages::ALIGN) as usize;
        let length = (num_pages * OwnedPages::ALIGN) as usize;

        buffer.truncate(0);
        let result = match self.bytes.lock().unwrap().get(offset..offset + length) {
            Some(bytes) => buffer
                .try_ensure_capacity(num_pages * OwnedPages::ALIGN)
                .and_then(|()| buffer.append(bytes))
                .map_err(StorageBackendError::from),
            None => Err(io::Error::other("Read out of bounds").into()),
        };
        sender.send((result, buffer)).unwrap();

        receiver
    }

    fn write(
        &self,
        buffer: SharedPages,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, SharedPages)> {
        let (sender, receiver) = oneshot::channel();
        assert!(buffer.len().is_multiple_of(SharedPages::ALIGN));

        if let Some(held_writes) = self.held_writes.lock().unwrap().as_mut() {
            held_writes.push(HeldWrite {
                buffer,
                offset,
                sender,
            });
        } else {
            sender.send(self.write_now(buffer, offset)).unwrap();
        }

        receiver
    }
}

impl MemoryStorageBackend {
    /// Create a new formatted storage backend
    pub(crate) fn new() -> Self {
        let storage_backend = Self {
            bytes: Arc::new(Mutex::new(vec![
                0;
                (NUM_PAGES * OwnedPages::ALIGN) as usize
            ])),
            remaining_writes: Arc::default(),
            held_writes: Arc::default(),
        };

        block_on(ClientDatabase::<OwnedBeaconChainBlock, Self>::format(
            &storage_backend,
            ClientDatabaseFormatOptions {
                page_group_size: PAGE_GROUP_SIZE,
                force: true,
            },
        ))
        .unwrap();

        storage_backend
    }

    /// Only let `num_writes` more writes reach the pages and silently drop the rest, `None` lets
    /// all writes through again
    pub(crate) fn limit_writes(&self, num_writes: Option<usize>) {
        *self.remaining_writes.lock().unwrap() = num_writes;
    }

    /// Hold back writes until [`Self::release_writes()`] or [`Self::fail_held_writes()`], as if
    /// they were still in flight
    pub(crate) fn hold_writes(&self) {
        self.held_writes.lock().unwrap().get_or_insert_default();
    }

    /// Let held writes reach the pages in the order they were submitted in and stop holding writes
    pub(crate) fn release_writes(&self) {
        let held_writes = self.held_writes.lock().unwrap().take().unwrap_or_default();
        for HeldWrite {
            buffer,
            offset,
            sender,
        } in held_writes
        {
            sender.send(self.write_now(buffer, offset)).unwrap();
        }
    }

    /// Fail held writes without them reaching the pages and stop holding writes
    pub(crate) fn fail_held_writes(&self) {
        let held_writes = self.held_writes.lock().unwrap().take().unwrap_or_default();
        for HeldWrite {
            buffer,
            offset: _,
            sender,
        } in held_writes
        {
            sender
                .send((Err(io::Error::other("Write failed").into()), buffer))
                .unwrap();
        }
    }

    fn write_now(
        &self,
        buffer: SharedPages,
        offset: u32,
    ) -> (Result<(), StorageBackendError>, SharedPages) {
        let offset = (offset * SharedPages::ALIGN) as usize;

        if let Some(remaining_writes) = self.remaining_writes.lock().unwrap().as_mut() {
            let Some(new_remaining_writes) = remaining_writes.checked_sub(1) else {
                // Lost as if the process was killed before the write reached the disk
                return (Ok(()), buffer);
            };
            *remaining_writes = new_remaining_writes;
        }

        let result = match self
            .bytes
            .lock()
            .unwrap()
            .get_mut(offset..offset + buffer.len() as usize)
        {
            Some(bytes) => {
                bytes.copy_from_slice(&buffer);
                Ok(())
            }
            None => Err(io::Error::other("Write out of bounds").into()),
        };

        (result, buffer)
    }
}

pub(crate) type Database = ClientDatabase<OwnedBeaconChainBlock, MemoryStorageBackend>;

fn consensus_info() -> BlockHeaderConsensusInfo {
    BlockHeaderConsensusInfo {
        slot: SlotNumber::ZERO,
        proof_of_time: PotOutput::default(),
        future_proof_of_time: PotOutput::default(),
        solution: Solution::genesis_solution(),
    }
}

fn seal() -> BlockHeaderEd25519Seal {
    BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::default(),
        signature: Ed25519Signature::default(),
    }
}

/// Beacon chain block with the body of `block_builder`, `salt` makes blocks with the same number,
/// parent and body unique
fn beacon_chain_block_with_body(
    block_builder: OwnedBeaconChainBlockBuilder,
    number: u64,
    parent_root: BlockRoot,
    salt: u64,
) -> OwnedBeaconChainBlock {
    block_builder
        .with_header(
            &BlockHeaderPrefix {
                number: BlockNumber::from(number),
                shard_index: ShardIndex::BEACON_CHAIN,
                padding_0: [0; _],
                timestamp: BlockTimestamp::from_millis(salt),
                parent_root,
                mmr_root: Blake3Hash::default(),
            },
            Blake3Hash::default(),
            &consensus_info(),
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
        .unwrap()
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
        .unwrap()
}

/// Minimal beacon chain block, `salt` makes blocks with the same number and parent unique
pub(crate) fn beacon_chain_block(
    number: u64,
    parent_root: BlockRoot,
    salt: u64,
) -> OwnedBeaconChainBlock {
    beacon_chain_block_with_body(
        OwnedBeaconChainBlock::init([].into_iter(), [].into_iter(), &[]).unwrap(),
        number,
        parent_root,
        salt,
    )
}

/// Segments of an intermediate shard block and of its leaf shard blocks
pub(crate) struct IntermediateShardBlockSegments<'a> {
    pub(crate) shard_index: ShardIndex,
    pub(crate) own_segments: &'a [(LocalSegmentIndex, SegmentRoot)],
    /// Shard indices and own segments of leaf shard blocks, segments can be empty
    pub(crate) leaf_shard_blocks: &'a [(ShardIndex, &'a [(LocalSegmentIndex, SegmentRoot)])],
}

/// Header prefix of a shard block of `shard_index` at `number`
fn shard_header_prefix(shard_index: ShardIndex, number: u64) -> BlockHeaderPrefix {
    BlockHeaderPrefix {
        number: BlockNumber::from(number),
        shard_index,
        padding_0: [0; _],
        timestamp: BlockTimestamp::default(),
        parent_root: BlockRoot::default(),
        mmr_root: Blake3Hash::default(),
    }
}

/// Info of an intermediate shard block at `number` with `segments`, whose leaf shard blocks are
/// numbered by their positions
fn intermediate_shard_block_info(
    number: u64,
    segments: &IntermediateShardBlockSegments<'_>,
) -> OwnedIntermediateShardBlockInfo {
    let beacon_chain_info = BlockHeaderBeaconChainInfo {
        number: BlockNumber::ZERO,
        root: BlockRoot::default(),
    };
    let leaf_shard_block_infos = (0..)
        .zip(segments.leaf_shard_blocks)
        .map(|(leaf_number, &(shard_index, own_segments))| {
            let block = OwnedLeafShardBlock::init(own_segments.iter().copied())
                .unwrap()
                .with_header(
                    &shard_header_prefix(shard_index, leaf_number),
                    Blake3Hash::default(),
                    &consensus_info(),
                    &beacon_chain_info,
                )
                .unwrap()
                .with_seal(BlockHeaderSeal::Ed25519(&seal()))
                .unwrap();

            OwnedLeafShardBlockInfo::from_block(&block.block())
        })
        .collect::<Vec<_>>();
    let block = OwnedIntermediateShardBlock::init(
        segments.own_segments.iter().copied(),
        leaf_shard_block_infos
            .iter()
            .map(|info| info.info().clone()),
    )
    .unwrap()
    .with_header(
        &shard_header_prefix(segments.shard_index, number),
        Blake3Hash::default(),
        &consensus_info(),
        &beacon_chain_info,
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
    .unwrap();

    OwnedIntermediateShardBlockInfo::from_block(&block.block())
}

/// Beacon chain block like [`beacon_chain_block()`] with zero salt, whose body has intermediate
/// shard blocks numbered by their positions with `intermediate_shard_blocks` segments
pub(crate) fn beacon_chain_block_with_shard_segments(
    number: u64,
    parent_root: BlockRoot,
    intermediate_shard_blocks: &[IntermediateShardBlockSegments<'_>],
) -> OwnedBeaconChainBlock {
    let intermediate_shard_block_infos = (0..)
        .zip(intermediate_shard_blocks)
        .map(|(intermediate_number, segments)| {
            intermediate_shard_block_info(intermediate_number, segments)
        })
        .collect::<Vec<_>>();

    beacon_chain_block_with_body(
        OwnedBeaconChainBlock::init(
            [].into_iter(),
            intermediate_shard_block_infos
                .iter()
                .map(|info| info.info().clone()),
            &[],
        )
        .unwrap(),
        number,
        parent_root,
        0,
    )
}

pub(crate) fn genesis_block_builder() -> GenesisBlockBuilderResult<OwnedBeaconChainBlock> {
    GenesisBlockBuilderResult {
        block: beacon_chain_block(0, BlockRoot::default(), 0),
        system_contract_states: [].into_iter().collect(),
    }
}

pub(crate) type DatabaseOptions = ClientDatabaseOptions<
    fn() -> GenesisBlockBuilderResult<OwnedBeaconChainBlock>,
    MemoryStorageBackend,
>;

/// Test options for a database on `storage_backend`
pub(crate) fn database_options(storage_backend: &MemoryStorageBackend) -> DatabaseOptions {
    ClientDatabaseOptions {
        write_buffer_size: 0,
        // Small enough to keep chains in tests short
        block_confirmation_depth: BlockNumber::from(5),
        soft_confirmation_depth: BlockNumber::from(3),
        max_fork_tips: NonZeroUsize::new(3).unwrap(),
        max_fork_tip_distance: BlockNumber::from(5),
        genesis_block_builder,
        storage_backend: storage_backend.clone(),
    }
}

/// Open the database with `options`
pub(crate) fn open_database_with_options(
    options: DatabaseOptions,
) -> Result<Database, ClientDatabaseError> {
    block_on(Database::open(options))
}

/// Open the database on `storage_backend` with the test options
pub(crate) fn open_database(storage_backend: &MemoryStorageBackend) -> Database {
    open_database_with_options(database_options(storage_backend)).unwrap()
}

/// Drop the database and open it again on the same storage backend with the same depths and fork
/// tip limits
pub(crate) fn reopen(database: Database, storage_backend: &MemoryStorageBackend) -> Database {
    let ClientDatabaseInnerOptions {
        block_confirmation_depth,
        soft_confirmation_depth,
        max_fork_tips,
        max_fork_tip_distance,
    } = database.inner.options;
    drop(database);

    open_database_with_options(DatabaseOptions {
        block_confirmation_depth,
        soft_confirmation_depth,
        max_fork_tips,
        max_fork_tip_distance,
        ..database_options(storage_backend)
    })
    .unwrap()
}

/// Block confirmation depth of `database`
pub(crate) fn block_confirmation_depth(database: &Database) -> u64 {
    u64::from(database.inner.options.block_confirmation_depth)
}

/// Persist `block` and return its root
pub(crate) fn persist_beacon_chain_block(
    database: &Database,
    block: OwnedBeaconChainBlock,
) -> Result<BlockRoot, PersistBlockError> {
    let block_root = *block.header.header().root();
    let block_details = BlockDetails {
        mmr_with_block: Arc::new(BlockMerkleMountainRange::new()),
        system_contract_states: [].into_iter().collect(),
    };

    block_on(database.persist_block(block, block_details))?;

    Ok(block_root)
}

/// Persist a block built by [`beacon_chain_block()`] and return its root
pub(crate) fn persist_block(
    database: &Database,
    number: u64,
    parent_root: BlockRoot,
    salt: u64,
) -> Result<BlockRoot, PersistBlockError> {
    persist_beacon_chain_block(database, beacon_chain_block(number, parent_root, salt))
}

/// Persist a chain of blocks with `numbers` on top of `parent_root`, returns their roots
pub(crate) fn persist_chain(
    database: &Database,
    mut parent_root: BlockRoot,
    numbers: RangeInclusive<u64>,
    salt: u64,
) -> Vec<BlockRoot> {
    numbers
        .map(|number| {
            parent_root = persist_block(database, number, parent_root, salt).unwrap();
            parent_root
        })
        .collect()
}

/// Highest block number of `database` that is persisted while the best block is at `best_number`
pub(crate) fn highest_persisted_number(database: &Database, best_number: u64) -> u64 {
    best_number - u64::from(database.inner.options.soft_confirmation_depth)
}

pub(crate) fn segment_header(local_segment_index: u64) -> SegmentHeader {
    SegmentHeader {
        index: LocalSegmentIndex::from(local_segment_index).into(),
        root: SegmentRoot::from([u8::try_from(local_segment_index).unwrap(); _]),
        prev_segment_header_hash: Blake3Hash::default(),
        last_archived_block: LastArchivedBlock {
            number: BlockNumber::from(local_segment_index).into(),
            archived_progress: ArchivedBlockProgress::new_complete(),
        },
    }
}

pub(crate) fn super_segment_header(super_segment_index: u64) -> SuperSegmentHeader {
    SuperSegmentHeader {
        index: SuperSegmentIndex::from(super_segment_index).into(),
        root: SuperSegmentRoot::from([u8::try_from(super_segment_index).unwrap(); _]),
        prev_super_segment_header_hash: Blake3Hash::default(),
        max_segment_index: SegmentIndex::from(super_segment_index).into(),
        target_beacon_chain_block_number: BlockNumber::from(super_segment_index).into(),
        num_segments: 1,
    }
}

/// Call `write` with the first `buffer_len` bytes of zeroed pages, which is how storage items are
/// written, and return its result together with the pages
pub(crate) fn write_to_aligned_pages<W>(
    buffer_len: usize,
    write: W,
) -> (Result<usize, StorageItemError>, SharedPages)
where
    W: FnOnce(&mut [MaybeUninit<u8>]) -> Result<usize, StorageItemError>,
{
    let num_bytes = u32::try_from(buffer_len)
        .unwrap()
        .next_multiple_of(OwnedPages::ALIGN);
    let mut pages = OwnedPages::zeroed(num_bytes);
    // SAFETY: Bytes are initialized and storage items only write initialized bytes
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            pages.as_mut_byte_ptr().cast::<MaybeUninit<u8>>(),
            buffer_len,
        )
    };
    let result = write(bytes);

    (result, pages.into_shared())
}
