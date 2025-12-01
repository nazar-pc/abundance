//! Client database.
//!
//! ## High-level architecture overview
//!
//! The database operates on [`ClientDatabaseStorageBackend`], which is backed by [`AlignedPage`]s
//! that can be read or written. Pages contain `StorageItem`s, one storage item can occupy one or
//! more pages, but pages always belong to a single storage item. Pages are the smallest unit and
//! align nicely with the hardware architecture of modern SSDs. Each page starts with a prefix that
//! describes the contents of the page. `StorageItem` always starts at the multiple of the
//! `u128`/16 bytes, allowing for direct memory mapping onto target data structures.
//!
//! [`AlignedPage`]: crate::storage_backend::AlignedPage
//!
//! Individual pages are grouped into page groups (configurable via [`ClientDatabaseOptions`]). Page
//! groups can be permanent and ephemeral. Permanent page groups store information that is never
//! going to be deleted, like segment headers. Ephemeral page groups store the majority of the
//! information about blocks, blockchain state and other things that are being created all the time.
//! Once information in an ephemeral page group is too old and no longer needed, it can be
//! repurposed for a new permanent or ephemeral page group. There are different kinds of page groups
//! defined in `PageGroupKind`, and each variant has independent sequence numbers.
//!
//! Page groups are append-only, there is only one active permanent and one ephemeral page group.
//! They are appended with more pages containing storage items until there is no space to add a
//! complete storage item, after which the next page group is started.
//!
//! Ephemeral page groups can be freed only when they contain 100% outdated storage items.
//! Individual pages can't be freed.
//!
//! Each storage item has a sequence number and checksums that help to define the global ordering
//! and check whether a storage item was written fully. Upon restart, the page group containing the
//! latest storage items is found, and the latest fully written storage item is identified to
//! reconstruct the database state.
//!
//! Each page group starts with a `StorageItemPageGroupHeader` storage item for easier
//! identification.
//!
//! The database is typically contained in a single file (though in principle could be contained in
//! multiple if necessary). Before the database can be used, it needs to be formatted with a
//! specific size (it is possible to increase the size afterward) before it can be used. It is
//! expected (but depends on the storage backend) that the whole file size is pre-allocated on disk
//! and no writes will fail due to lack of disk space (which could be the case with a sparse file).

#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![feature(
    default_field_values,
    get_mut_unchecked,
    iter_collect_into,
    maybe_uninit_as_bytes,
    maybe_uninit_fill,
    maybe_uninit_slice,
    maybe_uninit_write_slice,
    push_mut,
    try_blocks
)]

mod page_group;
pub mod storage_backend;
mod storage_backend_adapter;

use crate::page_group::block::StorageItemBlock;
use crate::page_group::block::block::StorageItemBlockBlock;
use crate::storage_backend::ClientDatabaseStorageBackend;
use crate::storage_backend_adapter::{
    StorageBackendAdapter, StorageItemHandlerArg, StorageItemHandlers, WriteLocation,
};
use ab_client_api::{
    BlockDetails, BlockMerkleMountainRange, ChainInfo, ChainInfoWrite, ContractSlotState,
    PersistBlockError, ReadBlockError,
};
use ab_core_primitives::block::body::owned::GenericOwnedBlockBody;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_io_type::trivial_type::TrivialType;
use async_lock::{
    RwLock as AsyncRwLock, RwLockUpgradableReadGuard, RwLockWriteGuard as AsyncRwLockWriteGuard,
};
use rand::rngs::OsError;
use rclite::Arc;
use replace_with::replace_with_or_abort;
use smallvec::{SmallVec, smallvec};
use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};
use std::num::{NonZeroU32, NonZeroUsize};
use std::ops::Deref;
use std::sync::Arc as StdArc;
use std::{fmt, io};
use tracing::error;

/// Unique identifier for a database
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(C)]
pub struct DatabaseId([u8; 32]);

impl Deref for DatabaseId {
    type Target = [u8; 32];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for DatabaseId {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl DatabaseId {
    #[inline(always)]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Default)]
struct BlockRootHasher(u64);

impl Hasher for BlockRootHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let Some(state) = bytes.as_chunks().0.first().copied().map(u64::from_le_bytes) else {
            return;
        };

        self.0 = state;
    }
}

#[derive(Debug)]
pub struct GenesisBlockBuilderResult<Block> {
    /// Genesis block
    pub block: Block,
    /// System contracts state in the genesis block
    pub system_contract_states: StdArc<[ContractSlotState]>,
}

/// Options for [`ClientDatabase`]
#[derive(Debug, Copy, Clone)]
pub struct ClientDatabaseOptions<GBB, StorageBackend> {
    /// Write buffer size.
    ///
    /// Larger buffer allows buffering more async writes for improved responsiveness but requires
    /// more RAM. Zero buffer size means all writes must be completed before returning from the
    /// operation that triggered it. Non-zero buffer means writes can happen in the background.
    ///
    /// The recommended value is 5.
    pub write_buffer_size: usize = 5,
    /// Blocks at this depth are considered to be "confirmed" and irreversible from the consensus
    /// perspective.
    ///
    /// This parameter allows establishing a final canonical order of blocks and eliminating any
    /// potential forks at a specified depth and beyond.
    pub confirmation_depth_k: BlockNumber,
    /// Soft confirmation depth for blocks.
    ///
    /// Doesn't prevent forking on the consensus level but makes it extremely unlikely.
    ///
    /// This parameter determines how many blocks are retained in memory before being written to
    /// disk. Writing discarded blocks to disk is a waste of resources, so they are retained in
    /// memory before being soft-confirmed and written to disk for longer-term storage.
    ///
    /// A smaller number reduces memory usage while increasing the probability of unnecessary disk
    /// writes. A larger number increases memory usage, while avoiding unnecessary disk writes, but
    /// also increases the chance of recent blocks not being retained on disk in case of a crash.
    ///
    /// The recommended value is 3 blocks.
    pub soft_confirmation_depth: BlockNumber = BlockNumber::new(3),
    /// Defines how many fork tips should be maintained in total.
    ///
    /// As natural forks occur, there may be more than one tip in existence, with only one of them
    /// being considered "canonical". This parameter defines how many of these tips to maintain in a
    /// sort of LRU style cache. Tips beyond this limit that were not extended for a long time will
    /// be pruned automatically.
    ///
    /// A larger number results in higher memory usage and higher complexity of pruning algorithms.
    ///
    /// The recommended value is 3 blocks.
    pub max_fork_tips: NonZeroUsize = NonZeroUsize::new(3).expect("Not zero; qed"),
    /// Max distance between fork tip and the best block.
    ///
    /// When forks are this deep, they will be pruned, even without reaching the `max_fork_tips`
    /// limit. This essentially means the tip was not extended for some time, and while it is
    /// theoretically possible for the chain to continue from this tip, the probability is so small
    /// that it is not worth storing it.
    ///
    /// A larger value results in higher memory usage and higher complexity of pruning algorithms.
    ///
    /// The recommended value is 5 blocks.
    pub max_fork_tip_distance: BlockNumber = BlockNumber::new(5),
    /// Genesis block builder is responsible to create genesis block and corresponding state for
    /// bootstrapping purposes.
    pub genesis_block_builder: GBB,
    /// Storage backend to use for storing and retrieving storage items
    pub storage_backend: StorageBackend,
}

/// Options for [`ClientDatabase`]
#[derive(Debug, Copy, Clone)]
pub struct ClientDatabaseFormatOptions {
    /// The number of [`AlignedPage`]s in a single page group.
    ///
    /// [`AlignedPage`]: crate::storage_backend::AlignedPage
    ///
    /// Each group always has a set of storage items with monotonically increasing sequence numbers.
    /// The database only frees page groups for reuse when all storage items there are no longer in
    /// use.
    ///
    /// A smaller number means storage can be reclaimed for reuse more quickly and higher
    /// concurrency during restart, but must not be too small that no storage item fits within a
    /// page group anymore. A larger number allows finding the range of sequence numbers that are
    /// already used and where potential write interruption happened on restart more efficiently,
    /// but will use more RAM in the process.
    ///
    /// The recommended size is 256 MiB unless a tiny database is used for testing purposes, where
    /// a smaller value might work too.
    pub page_group_size: NonZeroU32,
    /// By default, formatting will be aborted if the database appears to be already formatted.
    ///
    /// Setting this option to `true` skips the check and formats the database anyway.
    pub force: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientDatabaseError {
    /// Invalid soft confirmation depth, it must be smaller than confirmation depth k
    #[error("Invalid soft confirmation depth, it must be smaller than confirmation depth k")]
    InvalidSoftConfirmationDepth,
    /// Invalid max fork tip distance, it must be smaller or equal to confirmation depth k
    #[error("Invalid max fork tip distance, it must be smaller or equal to confirmation depth k")]
    InvalidMaxForkTipDistance,
    /// Storage backend has canceled read request
    #[error("Storage backend has canceled read request")]
    ReadRequestCancelled,
    /// Storage backend read error
    #[error("Storage backend read error: {error}")]
    ReadError {
        /// Low-level error
        error: io::Error,
    },
    /// Unsupported database version
    #[error("Unsupported database version: {database_version}")]
    UnsupportedDatabaseVersion {
        /// Database version
        database_version: u8,
    },
    /// Page group size is too small, must be at least two pages
    #[error("Page group size is too small ({page_group_size}), must be at least two pages")]
    PageGroupSizeTooSmall {
        /// Page group size in pages
        page_group_size: u32,
    },
    /// Unexpected sequence number
    #[error(
        "Unexpected sequence number {actual} at page offset {page_offset} (expected \
        {expected})"
    )]
    UnexpectedSequenceNumber {
        /// Sequence number in the database
        actual: u64,
        /// Expected sequence number
        expected: u64,
        /// Page offset where storage item is found
        page_offset: u32,
    },
    /// Unexpected storage item
    #[error("Unexpected storage item at offset {page_offset}: {storage_item:?}")]
    UnexpectedStorageItem {
        /// First storage item
        storage_item: Box<dyn fmt::Debug + Send + Sync>,
        /// Page offset where storage item is found
        page_offset: u32,
    },
    /// Invalid block
    #[error("Invalid block at offset {page_offset}")]
    InvalidBlock {
        /// Page offset where storage item is found
        page_offset: u32,
    },
    /// Failed to adjust ancestor block forks
    #[error("Failed to adjust ancestor block forks")]
    FailedToAdjustAncestorBlockForks,
    /// Database is not formatted yet
    #[error("Database is not formatted yet")]
    Unformatted,
    /// Non-permanent first page group
    #[error("Non-permanent first page group")]
    NonPermanentFirstPageGroup,
}

/// Error for [`ClientDatabase::format()`]
#[derive(Debug, thiserror::Error)]
pub enum ClientDatabaseFormatError {
    /// Storage backend has canceled read request
    #[error("Storage backend has canceled read request")]
    ReadRequestCancelled,
    /// Storage backend read error
    #[error("Storage backend read error: {error}")]
    ReadError {
        /// Low-level error
        error: io::Error,
    },
    /// Failed to generate database id
    #[error("Failed to generate database id")]
    FailedToGenerateDatabaseId {
        /// Low-level error
        #[from]
        error: OsError,
    },
    /// Database is already formatted yet
    #[error("Database is already formatted yet")]
    AlreadyFormatted,
    /// Storage backend has canceled a writing request
    #[error("Storage backend has canceled a writing request")]
    WriteRequestCancelled,
    /// Storage item write error
    #[error("Storage item write error")]
    StorageItemWriteError {
        /// Low-level error
        #[from]
        error: io::Error,
    },
}

#[derive(Debug, Copy, Clone)]
struct ForkTip {
    number: BlockNumber,
    root: BlockRoot,
}

#[derive(Debug)]
struct ClientDatabaseBlockInMemory<Block>
where
    Block: GenericOwnedBlock,
{
    block: Block,
    block_details: BlockDetails,
}

enum FullBlock<'a, Block>
where
    Block: GenericOwnedBlock,
{
    InMemory(&'a Block),
    Persisted {
        header: &'a Block::Header,
        write_location: WriteLocation,
    },
}

/// Client database block contains details about the block state in the database.
///
/// Originally all blocks are stored in memory. Once a block is soft-confirmed (see
/// [`ClientDatabaseOptions::soft_confirmation_depth`]), it is persisted (likely on disk). Later
///  when it is "confirmed" fully (see [`ClientDatabaseOptions::soft_confirmation_depth`]), it becomes
/// irreversible.
#[derive(Debug)]
enum ClientDatabaseBlock<Block>
where
    Block: GenericOwnedBlock,
{
    /// Block is stored in memory and wasn't persisted yet
    InMemory(ClientDatabaseBlockInMemory<Block>),
    /// Block was persisted (likely on disk)
    Persisted {
        header: Block::Header,
        block_details: BlockDetails,
        write_location: WriteLocation,
    },
    /// Block was persisted (likely on disk) and is irreversibly "confirmed" from the consensus
    /// perspective
    PersistedConfirmed {
        header: Block::Header,
        write_location: WriteLocation,
    },
}

impl<Block> ClientDatabaseBlock<Block>
where
    Block: GenericOwnedBlock,
{
    #[inline(always)]
    fn header(&self) -> &Block::Header {
        match self {
            Self::InMemory(in_memory) => in_memory.block.header(),
            Self::Persisted { header, .. } => header,
            Self::PersistedConfirmed { header, .. } => header,
        }
    }

    #[inline(always)]
    fn full_block(&self) -> FullBlock<'_, Block> {
        match self {
            Self::InMemory(in_memory) => FullBlock::InMemory(&in_memory.block),
            Self::Persisted {
                header,
                write_location,
                ..
            }
            | Self::PersistedConfirmed {
                header,
                write_location,
            } => FullBlock::Persisted {
                header,
                write_location: *write_location,
            },
        }
    }

    #[inline(always)]
    fn block_details(&self) -> Option<&BlockDetails> {
        match self {
            Self::InMemory(in_memory) => Some(&in_memory.block_details),
            Self::Persisted { block_details, .. } => Some(block_details),
            Self::PersistedConfirmed { .. } => None,
        }
    }
}

#[derive(Debug)]
struct StateData<Block>
where
    Block: GenericOwnedBlock,
{
    /// Tips of forks that have no descendants.
    ///
    /// The current best block is at the front, the rest are in the order from most recently updated
    /// towards the front to least recently at the back.
    fork_tips: VecDeque<ForkTip>,
    /// Map from block root to block number.
    ///
    /// Is meant to be used in conjunction with `headers` and `blocks` fields, which are indexed by
    /// block numbers.
    block_roots: HashMap<BlockRoot, BlockNumber, BuildHasherDefault<BlockRootHasher>>,
    /// List of blocks with the newest at the front.
    ///
    /// The first element of the first entry corresponds to the best block.
    ///
    /// It is expected that in most block numbers there will be exactly one block, some two,
    /// anything more than that will be very rare. The list of forks for a block number is organized
    /// in such a way that the first entry at every block number corresponds to the canonical
    /// version of the blockchain at any point in time.
    ///
    /// A position withing this data structure is called "block offset". This is an ephemeral value
    /// and changes as new best blocks are added. Blocks at the same height are collectively called
    /// "block forks" and the position of the block within the same block height is called
    /// "fork offset". While fork offset `0` always corresponds to the canonical version of the
    /// blockchain, other offsets are not guaranteed to follow any particular ordering rules.
    blocks: VecDeque<SmallVec<[ClientDatabaseBlock<Block>; 2]>>,
}

// TODO: Hide implementation details
#[derive(Debug)]
struct State<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    data: StateData<Block>,
    storage_backend_adapter: AsyncRwLock<StorageBackendAdapter<StorageBackend>>,
}

impl<Block, StorageBackend> State<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    #[inline(always)]
    fn best_tip(&self) -> &ForkTip {
        self.data
            .fork_tips
            .front()
            .expect("The best block is always present; qed")
    }

    #[inline(always)]
    fn best_block(&self) -> &ClientDatabaseBlock<Block> {
        self.data
            .blocks
            .front()
            .expect("The best block is always present; qed")
            .first()
            .expect("The best block is always present; qed")
    }
}

#[derive(Debug)]
struct BlockToPersist<'a, Block>
where
    Block: GenericOwnedBlock,
{
    block_offset: usize,
    fork_offset: usize,
    block: &'a ClientDatabaseBlockInMemory<Block>,
}

#[derive(Debug)]
struct PersistedBlock {
    block_offset: usize,
    fork_offset: usize,
    write_location: WriteLocation,
}

#[derive(Debug)]
struct ClientDatabaseInnerOptions {
    confirmation_depth_k: BlockNumber,
    soft_confirmation_depth: BlockNumber,
    max_fork_tips: NonZeroUsize,
    max_fork_tip_distance: BlockNumber,
}

#[derive(Debug)]
struct Inner<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    state: AsyncRwLock<State<Block, StorageBackend>>,
    options: ClientDatabaseInnerOptions,
}

/// Client database
#[derive(Debug)]
pub struct ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    inner: Arc<Inner<Block, StorageBackend>>,
}

impl<Block, StorageBackend> Clone for ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Block, StorageBackend> Drop for ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
{
    fn drop(&mut self) {
        // TODO: Persist things that were not persisted yet to reduce the data loss on shutdown
    }
}

impl<Block, StorageBackend> ChainInfo<Block> for ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
    StorageBackend: ClientDatabaseStorageBackend,
{
    fn best_root(&self) -> BlockRoot {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        self.inner.state.read_blocking().best_tip().root
    }

    fn best_header(&self) -> Block::Header {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        self.inner
            .state
            .read_blocking()
            .best_block()
            .header()
            .clone()
    }

    fn best_header_with_details(&self) -> (Block::Header, BlockDetails) {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        let state = self.inner.state.read_blocking();
        let best_block = state.best_block();
        (
            best_block.header().clone(),
            best_block
                .block_details()
                .expect("Always present for the best block; qed")
                .clone(),
        )
    }

    // TODO: Add fast path when `descendant_block_root` is the best block
    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<Block::Header> {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        let state = self.inner.state.read_blocking();
        let best_number = state.best_tip().number;

        let ancestor_block_offset =
            best_number.checked_sub(ancestor_block_number)?.as_u64() as usize;
        let ancestor_block_candidates = state.data.blocks.get(ancestor_block_offset)?;

        let descendant_block_number = *state.data.block_roots.get(descendant_block_root)?;
        if ancestor_block_number > descendant_block_number {
            return None;
        }
        let descendant_block_offset =
            best_number.checked_sub(descendant_block_number)?.as_u64() as usize;

        // Range of blocks where the first item is expected to contain a descendant
        let mut blocks_range_iter = state
            .data
            .blocks
            .iter()
            .enumerate()
            .skip(descendant_block_offset);

        let (_offset, descendant_block_candidates) = blocks_range_iter.next()?;
        let descendant_header = descendant_block_candidates
            .iter()
            .find(|block| &*block.header().header().root() == descendant_block_root)?
            .header()
            .header();

        // If there are no forks at this level, then this is the canonical chain and ancestor
        // block number we're looking for is the first block at the corresponding block number.
        // Similarly, if there is just a single ancestor candidate and descendant exists, it must be
        // the one we care about.
        if descendant_block_candidates.len() == 1 || ancestor_block_candidates.len() == 1 {
            return ancestor_block_candidates
                .iter()
                .next()
                .map(|block| block.header().clone());
        }

        let mut parent_block_root = &descendant_header.prefix.parent_root;

        // Iterate over the blocks following descendant until ancestor is reached
        for (block_offset, parent_candidates) in blocks_range_iter {
            let parent_header = parent_candidates
                .iter()
                .find(|header| &*header.header().header().root() == parent_block_root)?
                .header();

            // When header offset matches, we found the header
            if block_offset == ancestor_block_offset {
                return Some(parent_header.clone());
            }

            parent_block_root = &parent_header.header().prefix.parent_root;
        }

        None
    }

    fn header(&self, block_root: &BlockRoot) -> Option<Block::Header> {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        let state = self.inner.state.read_blocking();
        let best_number = state.best_tip().number;

        let block_number = *state.data.block_roots.get(block_root)?;
        let block_offset = best_number.checked_sub(block_number)?.as_u64() as usize;
        let block_candidates = state.data.blocks.get(block_offset)?;

        block_candidates.iter().find_map(|block| {
            let header = block.header();

            if &*header.header().root() == block_root {
                Some(header.clone())
            } else {
                None
            }
        })
    }

    fn header_with_details(&self, block_root: &BlockRoot) -> Option<(Block::Header, BlockDetails)> {
        // Blocking read lock is fine because the only place where write lock is taken is short and
        // all other locks are read locks
        let state = self.inner.state.read_blocking();
        let best_number = state.best_tip().number;

        let block_number = *state.data.block_roots.get(block_root)?;
        let block_offset = best_number.checked_sub(block_number)?.as_u64() as usize;
        let block_candidates = state.data.blocks.get(block_offset)?;

        block_candidates.iter().find_map(|block| {
            let header = block.header();
            let block_details = block.block_details().cloned()?;

            if &*header.header().root() == block_root {
                Some((header.clone(), block_details))
            } else {
                None
            }
        })
    }

    async fn block(&self, block_root: &BlockRoot) -> Result<Block, ReadBlockError> {
        let state = self.inner.state.read().await;
        let best_number = state.best_tip().number;

        let block_number = *state
            .data
            .block_roots
            .get(block_root)
            .ok_or(ReadBlockError::UnknownBlockRoot)?;
        let block_offset = best_number
            .checked_sub(block_number)
            .expect("Known block roots always have valid block offset; qed")
            .as_u64() as usize;
        let block_candidates = state
            .data
            .blocks
            .get(block_offset)
            .expect("Valid block offsets always have block entries; qed");

        for block_candidate in block_candidates {
            let header = block_candidate.header();

            if &*header.header().root() == block_root {
                return match block_candidate.full_block() {
                    FullBlock::InMemory(block) => Ok(block.clone()),
                    FullBlock::Persisted {
                        header,
                        write_location,
                    } => {
                        let storage_backend_adapter = state.storage_backend_adapter.read().await;

                        let storage_item = storage_backend_adapter
                            .read_storage_item::<StorageItemBlock>(write_location)
                            .await?;

                        #[expect(
                            clippy::infallible_destructuring_match,
                            reason = "Only a single variant for now"
                        )]
                        let storage_item_block = match storage_item {
                            StorageItemBlock::Block(storage_item_block) => storage_item_block,
                        };

                        let StorageItemBlockBlock {
                            header: _,
                            body,
                            mmr_with_block: _,
                            system_contract_states: _,
                        } = storage_item_block;

                        Block::from_buffers(header.buffer().clone(), body)
                            .ok_or(ReadBlockError::FailedToDecode)
                    }
                };
            }
        }

        unreachable!("Known block root always has block candidate associated with it; qed")
    }
}

impl<Block, StorageBackend> ChainInfoWrite<Block> for ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
    StorageBackend: ClientDatabaseStorageBackend,
{
    async fn persist_block(
        &self,
        block: Block,
        block_details: BlockDetails,
    ) -> Result<(), PersistBlockError> {
        let mut state = self.inner.state.write().await;
        let best_number = state.best_tip().number;

        let header = block.header().header();

        let block_number = header.prefix.number;

        if best_number == BlockNumber::ZERO && block_number != BlockNumber::ONE {
            // Special case when syncing on top of the fresh database
            Self::insert_first_block(&mut state.data, block, block_details);

            return Ok(());
        }

        if block_number == best_number + BlockNumber::ONE {
            return Self::insert_new_best_block(state, &self.inner, block, block_details).await;
        }

        let block_offset = best_number
            .checked_sub(block_number)
            .ok_or(PersistBlockError::MissingParent)?
            .as_u64() as usize;

        if block_offset >= self.inner.options.confirmation_depth_k.as_u64() as usize {
            return Err(PersistBlockError::OutsideAcceptableRange);
        }

        let state = &mut *state;

        let block_forks = state.data.blocks.get_mut(block_offset).ok_or_else(|| {
            error!(
                %block_number,
                %block_offset,
                "Failed to store block fork, header offset is missing despite being within \
                acceptable range"
            );

            PersistBlockError::OutsideAcceptableRange
        })?;

        for (index, fork_tip) in state.data.fork_tips.iter_mut().enumerate() {
            // Block's parent is no longer a fork tip, remove it
            if fork_tip.root == header.prefix.parent_root {
                state.data.fork_tips.remove(index);
                break;
            }
        }

        let block_root = *header.root();
        // Insert at position 1, which means the most recent tip, which doesn't correspond to
        // the best block
        state.data.fork_tips.insert(
            1,
            ForkTip {
                number: block_number,
                root: block_root,
            },
        );
        state.data.block_roots.insert(block_root, block_number);
        block_forks.push(ClientDatabaseBlock::InMemory(ClientDatabaseBlockInMemory {
            block,
            block_details,
        }));

        Self::prune_outdated_fork_tips(block_number, &mut state.data, &self.inner.options);

        Ok(())
    }
}

impl<Block, StorageBackend> ClientDatabase<Block, StorageBackend>
where
    Block: GenericOwnedBlock,
    StorageBackend: ClientDatabaseStorageBackend,
{
    /// Open the existing database.
    ///
    /// NOTE: The database needs to be formatted with [`Self::format()`] before it can be used.
    pub async fn open<GBB>(
        options: ClientDatabaseOptions<GBB, StorageBackend>,
    ) -> Result<Self, ClientDatabaseError>
    where
        GBB: FnOnce() -> GenesisBlockBuilderResult<Block>,
    {
        let ClientDatabaseOptions {
            write_buffer_size,
            confirmation_depth_k,
            soft_confirmation_depth,
            max_fork_tips,
            max_fork_tip_distance,
            genesis_block_builder,
            storage_backend,
        } = options;
        if soft_confirmation_depth >= confirmation_depth_k {
            return Err(ClientDatabaseError::InvalidSoftConfirmationDepth);
        }

        if max_fork_tip_distance > confirmation_depth_k {
            return Err(ClientDatabaseError::InvalidMaxForkTipDistance);
        }

        let mut state_data = StateData {
            fork_tips: VecDeque::new(),
            block_roots: HashMap::default(),
            blocks: VecDeque::new(),
        };

        let options = ClientDatabaseInnerOptions {
            confirmation_depth_k,
            soft_confirmation_depth,
            max_fork_tips,
            max_fork_tip_distance,
        };

        let storage_item_handlers = StorageItemHandlers {
            permanent: |_arg| {
                // TODO
                Ok(())
            },
            block: |arg| {
                let StorageItemHandlerArg {
                    storage_item,
                    page_offset,
                    num_pages,
                } = arg;
                #[expect(
                    clippy::infallible_destructuring_match,
                    reason = "Only a single variant for now"
                )]
                let storage_item_block = match storage_item {
                    StorageItemBlock::Block(storage_item_block) => storage_item_block,
                };

                // TODO: It would be nice to not allocate body here since we'll not use it here
                //  anyway
                let StorageItemBlockBlock {
                    header,
                    body: _,
                    mmr_with_block,
                    system_contract_states,
                } = storage_item_block;

                let header = Block::Header::from_buffer(header).map_err(|_buffer| {
                    error!(%page_offset, "Failed to decode block header from bytes");

                    ClientDatabaseError::InvalidBlock { page_offset }
                })?;

                let block_root = *header.header().root();
                let block_number = header.header().prefix.number;

                state_data.block_roots.insert(block_root, block_number);

                let maybe_best_number = state_data
                    .blocks
                    .front()
                    .and_then(|block_forks| block_forks.first())
                    .map(|best_block| {
                        // Type inference is not working here for some reason
                        let header: &Block::Header = best_block.header();

                        header.header().prefix.number
                    });

                let block_offset = if let Some(best_number) = maybe_best_number {
                    if block_number <= best_number {
                        (best_number - block_number).as_u64() as usize
                    } else {
                        // The new best block must follow the previous best block
                        if block_number - best_number != BlockNumber::ONE {
                            error!(
                                %page_offset,
                                %best_number,
                                %block_number,
                                "Invalid new best block number, it must be only one block \
                                higher than the best block"
                            );

                            return Err(ClientDatabaseError::InvalidBlock { page_offset });
                        }

                        state_data.blocks.push_front(SmallVec::new());
                        // Will insert a new block at the front
                        0
                    }
                } else {
                    state_data.blocks.push_front(SmallVec::new());
                    // Will insert a new block at the front
                    0
                };

                let block_forks = match state_data.blocks.get_mut(block_offset) {
                    Some(block_forks) => block_forks,
                    None => {
                        // Ignore the older block, other blocks at its height were already pruned
                        // anyway

                        return Ok(());
                    }
                };

                // Push a new block to the end of the list, we'll fix it up later
                block_forks.push(ClientDatabaseBlock::Persisted {
                    header,
                    block_details: BlockDetails {
                        mmr_with_block,
                        system_contract_states,
                    },
                    write_location: WriteLocation {
                        page_offset,
                        num_pages,
                    },
                });

                // If a new block was inserted, confirm a new canonical block to prune extra
                // in-memory information
                if block_offset == 0 && block_forks.len() == 1 {
                    Self::confirm_canonical_block(block_number, &mut state_data, &options);
                }

                Ok(())
            },
        };

        let storage_backend_adapter =
            StorageBackendAdapter::open(write_buffer_size, storage_item_handlers, storage_backend)
                .await?;

        if let Some(best_block) = state_data.blocks.front().and_then(|block_forks| {
            // The best block is last in the list here because that is how it was inserted while
            // reading from the database
            block_forks.last()
        }) {
            // Type inference is not working here for some reason
            let header: &Block::Header = best_block.header();
            let header = header.header();
            let block_number = header.prefix.number;
            let block_root = *header.root();

            if !Self::adjust_ancestor_block_forks(&mut state_data.blocks, block_root) {
                return Err(ClientDatabaseError::FailedToAdjustAncestorBlockForks);
            }

            // Store the best block as the first and only fork tip
            state_data.fork_tips.push_front(ForkTip {
                number: block_number,
                root: block_root,
            });
        } else {
            let GenesisBlockBuilderResult {
                block,
                system_contract_states,
            } = genesis_block_builder();

            // If the database is empty, initialize everything with the genesis block
            let header = block.header().header();
            let block_number = header.prefix.number;
            let block_root = *header.root();

            state_data.fork_tips.push_front(ForkTip {
                number: block_number,
                root: block_root,
            });
            state_data.block_roots.insert(block_root, block_number);
            state_data
                .blocks
                .push_front(smallvec![ClientDatabaseBlock::InMemory(
                    ClientDatabaseBlockInMemory {
                        block,
                        block_details: BlockDetails {
                            system_contract_states,
                            mmr_with_block: Arc::new({
                                let mut mmr = BlockMerkleMountainRange::new();
                                mmr.add_leaf(&block_root);
                                mmr
                            })
                        },
                    }
                )]);
        }

        let state = State {
            data: state_data,
            storage_backend_adapter: AsyncRwLock::new(storage_backend_adapter),
        };

        let inner = Inner {
            state: AsyncRwLock::new(state),
            options,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Format a new database
    pub async fn format(
        storage_backend: &StorageBackend,
        options: ClientDatabaseFormatOptions,
    ) -> Result<(), ClientDatabaseFormatError> {
        StorageBackendAdapter::format(storage_backend, options).await
    }

    fn insert_first_block(state: &mut StateData<Block>, block: Block, block_details: BlockDetails) {
        // If the database is empty, initialize everything with the genesis block
        let header = block.header().header();
        let block_number = header.prefix.number;
        let block_root = *header.root();

        state.fork_tips.clear();
        state.fork_tips.push_front(ForkTip {
            number: block_number,
            root: block_root,
        });
        state.block_roots.clear();
        state.block_roots.insert(block_root, block_number);
        state.blocks.clear();
        state
            .blocks
            .push_front(smallvec![ClientDatabaseBlock::InMemory(
                ClientDatabaseBlockInMemory {
                    block,
                    block_details,
                }
            )]);
    }

    async fn insert_new_best_block(
        mut state: AsyncRwLockWriteGuard<'_, State<Block, StorageBackend>>,
        inner: &Inner<Block, StorageBackend>,
        block: Block,
        block_details: BlockDetails,
    ) -> Result<(), PersistBlockError> {
        let header = block.header().header();
        let block_number = header.prefix.number;
        let block_root = *header.root();
        let parent_root = header.prefix.parent_root;

        // Adjust the relative order of forks to ensure the first index always corresponds to
        // ancestors of the new best block
        if !Self::adjust_ancestor_block_forks(&mut state.data.blocks, parent_root) {
            return Err(PersistBlockError::MissingParent);
        }

        // Store new block in the state
        {
            for (index, fork_tip) in state.data.fork_tips.iter_mut().enumerate() {
                // Block's parent is no longer a fork tip, remove it
                if fork_tip.root == parent_root {
                    state.data.fork_tips.remove(index);
                    break;
                }
            }

            state.data.fork_tips.push_front(ForkTip {
                number: block_number,
                root: block_root,
            });
            state.data.block_roots.insert(block_root, block_number);
            state
                .data
                .blocks
                .push_front(smallvec![ClientDatabaseBlock::InMemory(
                    ClientDatabaseBlockInMemory {
                        block,
                        block_details: block_details.clone()
                    }
                )]);
        }

        let options = &inner.options;

        Self::confirm_canonical_block(block_number, &mut state.data, options);
        Self::prune_outdated_fork_tips(block_number, &mut state.data, options);

        // Convert write lock into upgradable read lock to allow reads, while preventing concurrent
        // block modifications
        // TODO: This assumes both guarantees in https://github.com/smol-rs/async-lock/issues/100
        //  are satisfied. If not, blocking read locks in other places will cause issues.
        let state = AsyncRwLockWriteGuard::downgrade_to_upgradable(state);

        let mut blocks_to_persist = Vec::new();
        for block_offset in options.soft_confirmation_depth.as_u64() as usize.. {
            let Some(fork_blocks) = state.data.blocks.get(block_offset) else {
                break;
            };

            let len_before = blocks_to_persist.len();
            fork_blocks
                .iter()
                .enumerate()
                .filter_map(|(fork_offset, client_database_block)| {
                    match client_database_block {
                        ClientDatabaseBlock::InMemory(block) => Some(BlockToPersist {
                            block_offset,
                            fork_offset,
                            block,
                        }),
                        ClientDatabaseBlock::Persisted { .. }
                        | ClientDatabaseBlock::PersistedConfirmed { .. } => {
                            // Already persisted
                            None
                        }
                    }
                })
                .collect_into(&mut blocks_to_persist);

            if blocks_to_persist.len() == len_before {
                break;
            }
        }

        // Persist blocks from older to newer
        let mut persisted_blocks = Vec::with_capacity(blocks_to_persist.len());
        {
            let mut storage_backend_adapter = state.storage_backend_adapter.write().await;

            for block_to_persist in blocks_to_persist.into_iter().rev() {
                let BlockToPersist {
                    block_offset,
                    fork_offset,
                    block,
                } = block_to_persist;

                let write_location = storage_backend_adapter
                    .write_storage_item(StorageItemBlock::Block(StorageItemBlockBlock {
                        header: block.block.header().buffer().clone(),
                        body: block.block.body().buffer().clone(),
                        mmr_with_block: Arc::clone(&block.block_details.mmr_with_block),
                        system_contract_states: StdArc::clone(
                            &block.block_details.system_contract_states,
                        ),
                    }))
                    .await?;

                persisted_blocks.push(PersistedBlock {
                    block_offset,
                    fork_offset,
                    write_location,
                });
            }
        }

        // Convert blocks to persisted
        let mut state = RwLockUpgradableReadGuard::upgrade(state).await;
        for persisted_block in persisted_blocks {
            let PersistedBlock {
                block_offset,
                fork_offset,
                write_location,
            } = persisted_block;

            let block = state
                .data
                .blocks
                .get_mut(block_offset)
                .expect("Still holding the same lock since last check; qed")
                .get_mut(fork_offset)
                .expect("Still holding the same lock since last check; qed");

            replace_with_or_abort(block, |block| {
                if let ClientDatabaseBlock::InMemory(in_memory) = block {
                    let (header, _body) = in_memory.block.split();

                    ClientDatabaseBlock::Persisted {
                        header,
                        block_details: in_memory.block_details,
                        write_location,
                    }
                } else {
                    unreachable!("Still holding the same lock since last check; qed");
                }
            });
        }

        // TODO: Prune blocks that are no longer necessary
        // TODO: Prune unused page groups here or elsewhere?

        Ok(())
    }

    /// Adjust the relative order of forks to ensure the first index always corresponds to
    /// `parent_block_root` and its ancestors.
    ///
    /// Returns `true` on success and `false` if one of the parents was not found.
    #[must_use]
    fn adjust_ancestor_block_forks(
        blocks: &mut VecDeque<SmallVec<[ClientDatabaseBlock<Block>; 2]>>,
        mut parent_block_root: BlockRoot,
    ) -> bool {
        let mut ancestor_blocks = blocks.iter_mut();

        loop {
            if ancestor_blocks.len() == 1 {
                // Nothing left to adjust with a single fork
                break;
            }

            let Some(parent_blocks) = ancestor_blocks.next() else {
                // No more parent headers present
                break;
            };

            let Some(fork_offset_parent_block_root) =
                parent_blocks
                    .iter()
                    .enumerate()
                    .find_map(|(fork_offset, fork_block)| {
                        let fork_header = fork_block.header().header();
                        if *fork_header.root() == parent_block_root {
                            Some((fork_offset, fork_header.prefix.parent_root))
                        } else {
                            None
                        }
                    })
            else {
                return false;
            };

            let fork_offset;
            (fork_offset, parent_block_root) = fork_offset_parent_block_root;

            parent_blocks.swap(0, fork_offset);
        }

        true
    }

    /// Prune outdated fork tips that are too deep and have not been updated for a long time.
    ///
    /// Note that actual headers, blocks and MMRs could remain if they are currently used by
    /// something or were already persisted on disk. With persisted blocks specifically, RAM usage
    /// implications are minimal, and we wouldn't want to re-download already stored blocks in case
    /// they end up being necessary later.
    fn prune_outdated_fork_tips(
        best_number: BlockNumber,
        state: &mut StateData<Block>,
        options: &ClientDatabaseInnerOptions,
    ) {
        let state = &mut *state;

        // These forks are just candidates because they will not be pruned if the reference count is
        // not 1, indicating they are still in use by something
        let mut candidate_forks_to_remove = Vec::with_capacity(options.max_fork_tips.get());

        // Prune forks that are too far away from the best block
        state.fork_tips.retain(|fork_tip| {
            if best_number - fork_tip.number > options.max_fork_tip_distance {
                candidate_forks_to_remove.push(*fork_tip);
                false
            } else {
                true
            }
        });
        // Prune forks that exceed the maximum number of forks
        if state.fork_tips.len() > options.max_fork_tips.get() {
            state
                .fork_tips
                .drain(options.max_fork_tips.get()..)
                .collect_into(&mut candidate_forks_to_remove);
        }

        // Prune all possible candidates
        candidate_forks_to_remove
            .retain(|fork_tip| !Self::prune_outdated_fork(best_number, fork_tip, state));
        // Return those that were not pruned back to the list of tips
        state.fork_tips.extend(candidate_forks_to_remove);
    }

    /// Returns `true` if the tip was pruned successfully and `false` if it should be returned to
    /// the list of fork tips
    #[must_use]
    fn prune_outdated_fork(
        best_number: BlockNumber,
        fork_tip: &ForkTip,
        state: &mut StateData<Block>,
    ) -> bool {
        let block_offset = (best_number - fork_tip.number).as_u64() as usize;

        // Prune fork top and all its ancestors that are not used
        let mut block_root_to_prune = fork_tip.root;
        let mut pruned_tip = false;
        for block_offset in block_offset.. {
            let Some(fork_blocks) = state.blocks.get_mut(block_offset) else {
                if !pruned_tip {
                    error!(
                        %best_number,
                        ?fork_tip,
                        block_offset,
                        "Block offset was not present in the database, this is an implementation \
                        bug #1"
                    );
                }
                // No forks left to prune
                break;
            };

            if fork_blocks.len() == 1 {
                if !pruned_tip {
                    error!(
                        %best_number,
                        ?fork_tip,
                        block_offset,
                        "Block offset was not present in the database, this is an implementation \
                        bug #2"
                    );
                }

                // No forks left to prune
                break;
            }

            let Some((fork_offset, block)) = fork_blocks
                .iter()
                .enumerate()
                // Skip ancestor of the best block, it is certainly not a fork to be pruned
                .skip(1)
                .find(|(_fork_offset, block)| {
                    *block.header().header().root() == block_root_to_prune
                })
            else {
                if !pruned_tip {
                    error!(
                        %best_number,
                        ?fork_tip,
                        block_offset,
                        "Block offset was not present in the database, this is an implementation \
                        bug #3"
                    );
                }

                // Nothing left to prune
                break;
            };

            // More than one instance means something somewhere is using or depends on this block
            if block.header().ref_count() > 1 {
                break;
            }

            // Blocks that are already persisted
            match block {
                ClientDatabaseBlock::InMemory(_) => {
                    // Prune
                }
                ClientDatabaseBlock::Persisted { .. }
                | ClientDatabaseBlock::PersistedConfirmed { .. } => {
                    // Already on disk, keep it in memory for later, but prune the tip
                    pruned_tip = true;
                    break;
                }
            }

            state.block_roots.get_mut(&block_root_to_prune);
            block_root_to_prune = block.header().header().prefix.parent_root;
            fork_blocks.swap_remove(fork_offset);

            pruned_tip = true;
        }

        pruned_tip
    }

    /// Confirm a block at confirmation depth k and prune any other blocks at the same depth with
    /// their descendants
    fn confirm_canonical_block(
        best_number: BlockNumber,
        state_data: &mut StateData<Block>,
        options: &ClientDatabaseInnerOptions,
    ) {
        // `+1` means it effectively confirms parent blocks instead. This is done to keep the parent
        // of the confirmed block with its MMR in memory due to confirmed blocks not storing their
        // MMRs, which might be needed for reorgs at the lowest possible depth.
        let block_offset = (options.confirmation_depth_k + BlockNumber::ONE).as_u64() as usize;

        let Some(fork_blocks) = state_data.blocks.get_mut(block_offset) else {
            // Nothing to confirm yet
            return;
        };

        // Mark the canonical block as confirmed
        {
            let Some(canonical_block) = fork_blocks.first_mut() else {
                error!(
                    %best_number,
                    block_offset,
                    "Have not found a canonical block to confirm, this is an implementation bug"
                );
                return;
            };

            replace_with_or_abort(canonical_block, |block| match block {
                ClientDatabaseBlock::InMemory(_) => {
                    error!(
                        %best_number,
                        block_offset,
                        header = ?block.header(),
                        "Block to be confirmed must not be in memory, this is an implementation bug"
                    );
                    block
                }
                ClientDatabaseBlock::Persisted {
                    header,
                    block_details: _,
                    write_location,
                } => ClientDatabaseBlock::PersistedConfirmed {
                    header,
                    write_location,
                },
                ClientDatabaseBlock::PersistedConfirmed { .. } => {
                    error!(
                        %best_number,
                        block_offset,
                        header = ?block.header(),
                        "Block to be confirmed must not be confirmed yet, this is an \
                        implementation bug"
                    );
                    block
                }
            });
        }

        // Prune the rest of the blocks and their descendants
        let mut block_roots_to_prune = fork_blocks
            .drain(1..)
            .map(|block| *block.header().header().root())
            .collect::<Vec<_>>();
        let mut current_block_offset = block_offset;
        while !block_roots_to_prune.is_empty() {
            // Prune fork tips (if any)
            state_data
                .fork_tips
                .retain(|fork_tip| !block_roots_to_prune.contains(&fork_tip.root));

            // Prune removed block roots
            for block_root in &block_roots_to_prune {
                state_data.block_roots.remove(block_root);
            }

            // Block offset for direct descendants
            if let Some(next_block_offset) = current_block_offset.checked_sub(1) {
                current_block_offset = next_block_offset;
            } else {
                // Reached the tip
                break;
            }

            let fork_blocks = state_data
                .blocks
                .get_mut(current_block_offset)
                .expect("Lower block offset always exists; qed");

            // Collect descendants of pruned blocks to prune them next
            block_roots_to_prune = fork_blocks
                .drain_filter(|block| {
                    let header = block.header().header();

                    block_roots_to_prune.contains(&header.prefix.parent_root)
                })
                .map(|block| *block.header().header().root())
                .collect();
        }
    }
}
