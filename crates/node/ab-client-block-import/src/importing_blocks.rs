use crate::importing_blocks::private::ImportingBlockEntryInner;
use ab_client_api::{BlockMerkleMountainRange, ContractSlotState};
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use async_lock::{RwLock, RwLockWriteGuard};
use parking_lot::Mutex;
use rclite::Arc;
use stable_deref_trait::StableDeref;
use std::collections::VecDeque;
use std::mem;
use std::ops::Deref;
use std::sync::Arc as StdArc;
use yoke::{Yoke, Yokeable};

mod private {
    use ab_client_api::{BlockMerkleMountainRange, ContractSlotState};
    use ab_core_primitives::block::BlockRoot;
    use async_lock::RwLock;
    use rclite::Arc;
    use std::sync::Arc as StdArc;

    // Needs to be public to appear in `impl Deref for ImportingBlockEntry`
    #[derive(Debug)]
    pub struct ImportingBlockEntryInner<BlockHeader> {
        pub(super) block_root: BlockRoot,
        pub(super) header: BlockHeader,
        pub(super) mmr: Arc<BlockMerkleMountainRange>,
        pub(super) system_contract_states: RwLock<Option<StdArc<[ContractSlotState]>>>,
    }
}

#[derive(Debug)]
pub(crate) enum ParentBlockImportStatus<BlockHeader> {
    Importing {
        entry: ImportingBlockEntry<BlockHeader>,
    },
    Imported {
        system_contract_states: StdArc<[ContractSlotState]>,
    },
}

impl<BlockHeader> ParentBlockImportStatus<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    /// Check if the corresponding block import has failed
    pub(crate) fn has_failed(&self) -> bool {
        match self {
            Self::Importing { entry } => entry.has_failed(),
            Self::Imported { .. } => false,
        }
    }

    /// Wait for the corresponding block to be imported.
    ///
    /// Returns `Some(system_contract_states)` if a block was imported successfully and `None`
    /// otherwise.
    pub(crate) async fn wait(self) -> Option<StdArc<[ContractSlotState]>> {
        match self {
            Self::Importing { entry } => entry.wait().await,
            Self::Imported {
                system_contract_states,
            } => Some(system_contract_states),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImportingBlockEntry<BlockHeader> {
    inner: Arc<ImportingBlockEntryInner<BlockHeader>>,
}

impl<BlockHeader> Deref for ImportingBlockEntry<BlockHeader> {
    type Target = ImportingBlockEntryInner<BlockHeader>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// SAFETY: Heap-allocated data structure, points to the same memory if moved
unsafe impl<BlockHeader> StableDeref for ImportingBlockEntry<BlockHeader> {}

impl<BlockHeader> ImportingBlockEntry<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    fn new(header: BlockHeader, mmr: Arc<BlockMerkleMountainRange>) -> Self {
        let block_root = *header.header().root();

        Self {
            inner: Arc::new(ImportingBlockEntryInner {
                block_root,
                header,
                mmr,
                system_contract_states: RwLock::new(None),
            }),
        }
    }

    #[inline(always)]
    fn block_root(&self) -> &BlockRoot {
        &self.inner.block_root
    }

    #[inline(always)]
    pub(crate) fn header(&self) -> &BlockHeader {
        &self.inner.header
    }

    #[inline(always)]
    pub(crate) fn mmr(&self) -> &Arc<BlockMerkleMountainRange> {
        &self.inner.mmr
    }

    /// Check if the corresponding block import has failed
    #[inline(always)]
    pub(crate) fn has_failed(&self) -> bool {
        match self.inner.system_contract_states.try_read() {
            Some(success) => success.is_none(),
            None => false,
        }
    }

    /// Wait for the corresponding block to be imported.
    ///
    /// Returns `Some(system_contract_states)` if a block was imported successfully and `None`
    /// otherwise.
    pub(crate) async fn wait(self) -> Option<StdArc<[ContractSlotState]>> {
        self.inner
            .system_contract_states
            .read()
            .await
            .as_ref()
            .cloned()
    }
}

// The only reason this wrapper exists is to be able to implement [`Yokeable`] on it
#[derive(Debug)]
struct ImportingBlockHandleGuard<'a>(RwLockWriteGuard<'a, Option<StdArc<[ContractSlotState]>>>);

// SAFETY: Lifetime parameter on `RwLockWriteGuard` is covariant
unsafe impl<'a> Yokeable<'a> for ImportingBlockHandleGuard<'static> {
    type Output = ImportingBlockHandleGuard<'a>;

    #[inline(always)]
    fn transform(&'a self) -> &'a ImportingBlockHandleGuard<'a> {
        self
    }

    #[inline(always)]
    fn transform_owned(self) -> ImportingBlockHandleGuard<'a> {
        self
    }

    #[inline(always)]
    unsafe fn make(from: ImportingBlockHandleGuard<'a>) -> Self {
        // SAFETY: Implementation is a `transmute`, as per in `Yokeable::make()`'s description
        unsafe { mem::transmute(from) }
    }

    #[inline(always)]
    fn transform_mut<F>(&'a mut self, f: F)
    where
        F: FnOnce(&'a mut Self::Output) + 'static,
    {
        // SAFETY: Implementation is a `transmute`, as per in `Yokeable::transform_mut()`'s
        // description
        f(unsafe { mem::transmute::<&mut Self, &mut Self::Output>(self) })
    }
}

/// A handle to block that is being imported.
///
/// The corresponding entry will be removed from [`ImportingBlocks<BlockHeader>`] when this instance
/// is dropped.
#[derive(Debug)]
pub(crate) struct ImportingBlockHandle<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    guard: Yoke<ImportingBlockHandleGuard<'static>, ImportingBlockEntry<BlockHeader>>,
    importing_blocks: ImportingBlocks<BlockHeader>,
}

impl<BlockHeader> Deref for ImportingBlockHandle<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    type Target = ImportingBlockEntry<BlockHeader>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.guard.backing_cart()
    }
}

impl<BlockHeader> Drop for ImportingBlockHandle<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    #[inline]
    fn drop(&mut self) {
        let block_root = self.guard.backing_cart().block_root();
        self.importing_blocks.remove(block_root);
    }
}

impl<BlockHeader> ImportingBlockHandle<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    #[inline]
    fn new(
        entry: ImportingBlockEntry<BlockHeader>,
        importing_blocks: ImportingBlocks<BlockHeader>,
    ) -> Self {
        Self {
            guard: Yoke::attach_to_cart(entry, |entry_inner| {
                ImportingBlockHandleGuard(entry_inner.system_contract_states.write_blocking())
            }),
            importing_blocks,
        }
    }

    /// Set system contract states, indicating that the import of the corresponding block has
    /// finished successfully
    #[inline(always)]
    pub(crate) fn set_success(mut self, system_contract_states: StdArc<[ContractSlotState]>) {
        self.guard.with_mut(|guard| {
            guard.0.replace(system_contract_states);
        });
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImportingBlocks<BlockHeader> {
    list: Arc<Mutex<VecDeque<ImportingBlockEntry<BlockHeader>>>>,
}

impl<BlockHeader> ImportingBlocks<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            list: Arc::default(),
        }
    }

    /// Insert a block of the header that is being imported.
    ///
    /// Returned header is used to indicate a successful import of the block. If this block is
    /// already being imported, `None` is returned.
    pub(crate) fn insert(
        &self,
        header: BlockHeader,
        mmr: Arc<BlockMerkleMountainRange>,
    ) -> Option<ImportingBlockHandle<BlockHeader>> {
        let new_entry = ImportingBlockEntry::new(header, mmr);
        let handle = ImportingBlockHandle::new(new_entry.clone(), self.clone());

        let mut list = self.list.lock();
        if list
            .iter()
            .any(|entry| entry.block_root() == new_entry.block_root())
        {
            return None;
        }
        list.push_back(new_entry);

        Some(handle)
    }

    fn remove(&self, block_root: &BlockRoot) {
        let mut list = self.list.lock();

        // The front element is the most likely to be removed, though not guaranteed
        if let Some(entry_index) = list
            .iter()
            .enumerate()
            .find_map(|(index, entry)| (entry.block_root() == block_root).then_some(index))
        {
            list.swap_remove_front(entry_index);
        }
    }

    pub(crate) fn get(&self, block_root: &BlockRoot) -> Option<ImportingBlockEntry<BlockHeader>> {
        // The back element is the most likely one to be returned, though it is not guaranteed
        self.list
            .lock()
            .iter()
            .rev()
            .find(|entry| entry.block_root() == block_root)
            .cloned()
    }
}
