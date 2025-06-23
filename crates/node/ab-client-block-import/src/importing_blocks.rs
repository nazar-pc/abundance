use crate::importing_blocks::private::ImportingBlockEntryInner;
use ab_client_api::BlockMerkleMountainRange;
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
use yoke::{Yoke, Yokeable};

mod private {
    use ab_client_api::BlockMerkleMountainRange;
    use ab_core_primitives::block::BlockRoot;
    use async_lock::RwLock;
    use rclite::Arc;

    // Needs to be public to appear in `impl Deref for ImportingBlockEntry`
    #[derive(Debug)]
    pub struct ImportingBlockEntryInner<BlockHeader> {
        pub(super) block_root: BlockRoot,
        pub(super) header: BlockHeader,
        pub(super) mmr: Arc<BlockMerkleMountainRange>,
        pub(super) success: RwLock<bool>,
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
                success: RwLock::new(false),
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

    /// Check if corresponding block import has failed
    #[inline(always)]
    pub(crate) fn has_failed(&self) -> bool {
        match self.inner.success.try_read() {
            Some(success) => !*success,
            None => false,
        }
    }

    /// Wait for corresponding block to be imported.
    ///
    /// Returns `true` if block was imported successfully.
    pub(crate) async fn wait_success(self) -> bool {
        *self.inner.success.read().await
    }
}

// The only reason this wrapper exists is to be able to implement [`Yokeable`] on it
#[derive(Debug)]
struct ImportingBlockHandleGuard<'a>(RwLockWriteGuard<'a, bool>);

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
/// Corresponding entry will be removed from [`ImportingBlocks<BlockHeader>`] when this instance is
/// dropped.
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
                ImportingBlockHandleGuard(entry_inner.success.write_blocking())
            }),
            importing_blocks,
        }
    }

    /// Indicate that the import of the corresponding block has finished successfully
    #[inline(always)]
    pub(crate) fn set_success(mut self) {
        self.guard.with_mut(|guard| {
            *guard.0 = true;
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
    /// Returned header is used to indicate successful import of the block. If this block is already
    /// being imported, `None` is returned.
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

        // Front element is the most likely to be removed, though not guaranteed
        if let Some(entry_index) = list
            .iter()
            .enumerate()
            .find_map(|(index, entry)| (entry.block_root() == block_root).then_some(index))
        {
            list.swap_remove_front(entry_index);
        }
    }

    pub(crate) fn get(&self, block_root: &BlockRoot) -> Option<ImportingBlockEntry<BlockHeader>> {
        // Back element is the most likely one to be returned, though it is not guaranteed
        self.list
            .lock()
            .iter()
            .rev()
            .find(|entry| entry.block_root() == block_root)
            .cloned()
    }
}
