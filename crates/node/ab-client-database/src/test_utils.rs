use crate::storage_backend::{AlignedPage, ClientDatabaseStorageBackend};
use crate::storage_backend_adapter::storage_item::StorageItemError;
use crate::{
    ClientDatabase, ClientDatabaseFormatOptions, ClientDatabaseOptions, GenesisBlockBuilderResult,
};
use ab_client_api::{BlockDetails, ChainInfoWrite, PersistBlockError};
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
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
use std::{fmt, io};
use triomphe::Arc;

const NUM_PAGES: usize = 4096;
const PAGE_GROUP_SIZE: NonZeroU32 = NonZeroU32::new(256).unwrap();
// Small enough to keep chains in tests short
const BLOCK_CONFIRMATION_DEPTH: BlockNumber = BlockNumber::from(5);
const SOFT_CONFIRMATION_DEPTH: BlockNumber = BlockNumber::from(3);

/// In-memory storage backend, clones share the same pages, so a database can be reopened on them
/// after the previous instance was dropped
#[derive(Clone)]
pub(crate) struct MemoryStorageBackend {
    pages: Arc<Mutex<Vec<AlignedPage>>>,
}

impl fmt::Debug for MemoryStorageBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStorageBackend")
            .finish_non_exhaustive()
    }
}

impl ClientDatabaseStorageBackend for MemoryStorageBackend {
    fn num_pages(&self) -> u32 {
        u32::try_from(self.pages.lock().unwrap().len()).unwrap()
    }

    fn read(
        &self,
        mut buffer: Vec<AlignedPage>,
        length: u32,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>> {
        let (sender, receiver) = oneshot::channel();
        let offset = usize::try_from(offset).unwrap();
        let length = usize::try_from(length).unwrap();

        let result = match self.pages.lock().unwrap().get(offset..offset + length) {
            Some(pages) => {
                buffer.extend_from_slice(pages);
                Ok(buffer)
            }
            None => Err(io::Error::other("Read out of bounds")),
        };
        sender.send(result).unwrap();

        receiver
    }

    fn write(
        &self,
        buffer: Vec<AlignedPage>,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>> {
        let (sender, receiver) = oneshot::channel();
        let offset = usize::try_from(offset).unwrap();

        let result = match self
            .pages
            .lock()
            .unwrap()
            .get_mut(offset..offset + buffer.len())
        {
            Some(pages) => {
                pages.copy_from_slice(&buffer);
                Ok(buffer)
            }
            None => Err(io::Error::other("Write out of bounds")),
        };
        sender.send(result).unwrap();

        receiver
    }
}

impl MemoryStorageBackend {
    /// Create a new formatted storage backend
    pub(crate) fn new() -> Self {
        let storage_backend = Self {
            pages: Arc::new(Mutex::new(vec![AlignedPage::default(); NUM_PAGES])),
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
}

pub(crate) type Database = ClientDatabase<OwnedBeaconChainBlock, MemoryStorageBackend>;

/// Minimal beacon chain block, `salt` makes blocks with the same number and parent unique
fn beacon_chain_block(number: u64, parent_root: BlockRoot, salt: u64) -> OwnedBeaconChainBlock {
    OwnedBeaconChainBlock::init([].into_iter(), [].into_iter(), &[])
        .unwrap()
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

fn genesis_block_builder() -> GenesisBlockBuilderResult<OwnedBeaconChainBlock> {
    GenesisBlockBuilderResult {
        block: beacon_chain_block(0, BlockRoot::default(), 0),
        system_contract_states: [].into_iter().collect(),
    }
}

/// Open the database on `storage_backend` with the test options
pub(crate) fn open_database(storage_backend: &MemoryStorageBackend) -> Database {
    block_on(Database::open(ClientDatabaseOptions {
        write_buffer_size: 0,
        block_confirmation_depth: BLOCK_CONFIRMATION_DEPTH,
        soft_confirmation_depth: SOFT_CONFIRMATION_DEPTH,
        max_fork_tips: NonZeroUsize::new(3).unwrap(),
        max_fork_tip_distance: BlockNumber::from(5),
        genesis_block_builder,
        storage_backend: storage_backend.clone(),
    }))
    .unwrap()
}

/// Drop the database and open it again on the same storage backend
pub(crate) fn reopen(database: Database, storage_backend: &MemoryStorageBackend) -> Database {
    drop(database);
    open_database(storage_backend)
}

/// Persist a block built by [`beacon_chain_block()`] and return its root
pub(crate) fn persist_block(
    database: &Database,
    number: u64,
    parent_root: BlockRoot,
    salt: u64,
) -> Result<BlockRoot, PersistBlockError> {
    let block = beacon_chain_block(number, parent_root, salt);
    let block_root = *block.header.header().root();
    let block_details = BlockDetails {
        mmr_with_block: Arc::new(BlockMerkleMountainRange::new()),
        system_contract_states: [].into_iter().collect(),
    };

    block_on(database.persist_block(block, block_details))?;

    Ok(block_root)
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

/// Highest block number that is persisted while the best block is at `best_number`
pub(crate) fn highest_persisted_number(best_number: u64) -> u64 {
    best_number - u64::from(SOFT_CONFIRMATION_DEPTH)
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

/// Call `write` with the first `buffer_len` bytes of zeroed aligned pages, which is how storage
/// items are written, and return its result together with the pages
pub(crate) fn write_to_aligned_pages<W>(
    buffer_len: usize,
    write: W,
) -> (Result<usize, StorageItemError>, Box<[AlignedPage]>)
where
    W: FnOnce(&mut [MaybeUninit<u8>]) -> Result<usize, StorageItemError>,
{
    let num_pages = buffer_len.div_ceil(AlignedPage::SIZE);
    let mut pages = vec![MaybeUninit::new(AlignedPage::default()); num_pages].into_boxed_slice();
    let result = write(&mut pages.as_bytes_mut()[..buffer_len]);
    // SAFETY: All pages were initialized above and storage items only write initialized bytes
    let pages = unsafe { pages.assume_init() };

    (result, pages)
}
