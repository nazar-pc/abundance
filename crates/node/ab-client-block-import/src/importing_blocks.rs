use crate::importing_blocks::private::ImportingBlockEntryInner;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use async_lock::{RwLock, RwLockWriteGuard};
use parking_lot::Mutex;
use stable_deref_trait::StableDeref;
use std::collections::BTreeMap;
use std::mem;
use std::ops::Deref;
use std::sync::Arc;
use yoke::{Yoke, Yokeable};

mod private {
    use async_lock::RwLock;

    // Needs to be public to appear in `impl Deref for ImportingBlockEntry`
    #[derive(Debug)]
    pub struct ImportingBlockEntryInner<BlockHeader> {
        pub(super) header: BlockHeader,
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

impl<BlockHeader> ImportingBlockEntry<BlockHeader> {
    pub(crate) fn header(&self) -> &BlockHeader {
        &self.inner.header
    }

    /// Check if corresponding block import has failed
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

    fn transform(&'a self) -> &'a ImportingBlockHandleGuard<'a> {
        self
    }

    fn transform_owned(self) -> ImportingBlockHandleGuard<'a> {
        self
    }

    unsafe fn make(from: ImportingBlockHandleGuard<'a>) -> Self {
        // SAFETY: Implementation is a `transmute`, as per in `Yokeable::make()`'s description
        unsafe { mem::transmute(from) }
    }

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
pub(crate) struct ImportingBlockHandle<BlockHeader> {
    block_root: BlockRoot,
    guard: Yoke<ImportingBlockHandleGuard<'static>, ImportingBlockEntry<BlockHeader>>,
    map: Arc<Mutex<BTreeMap<BlockRoot, ImportingBlockEntry<BlockHeader>>>>,
}

impl<BlockHeader> Drop for ImportingBlockHandle<BlockHeader> {
    fn drop(&mut self) {
        self.map.lock().remove(&self.block_root);
    }
}

impl<BlockHeader> ImportingBlockHandle<BlockHeader> {
    /// Indicate that the import of the corresponding block has finished successfully
    pub(crate) fn set_success(mut self) {
        self.guard.with_mut(|guard| {
            *guard.0 = true;
        });
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImportingBlocks<BlockHeader> {
    map: Arc<Mutex<BTreeMap<BlockRoot, ImportingBlockEntry<BlockHeader>>>>,
}

impl<BlockHeader> ImportingBlocks<BlockHeader>
where
    BlockHeader: GenericOwnedBlockHeader,
{
    pub(crate) fn new() -> Self {
        Self {
            map: Arc::default(),
        }
    }

    /// Insert a block of the header that is being imported.
    ///
    /// Returned header is used to indicate successful import of the block. If this block is already
    /// being imported, `None` is returned.
    pub(crate) fn insert(&self, header: BlockHeader) -> Option<ImportingBlockHandle<BlockHeader>> {
        let block_root = *header.header().root();

        let entry = ImportingBlockEntry {
            inner: Arc::new(ImportingBlockEntryInner {
                header,
                success: RwLock::new(false),
            }),
        };
        let handle = ImportingBlockHandle {
            block_root,
            guard: Yoke::attach_to_cart(entry.clone(), |entry_inner| {
                ImportingBlockHandleGuard(entry_inner.success.write_blocking())
            }),
            map: Arc::clone(&self.map),
        };

        self.map.lock().try_insert(block_root, entry).ok()?;

        Some(handle)
    }

    pub(crate) fn get(&self, block_root: &BlockRoot) -> Option<ImportingBlockEntry<BlockHeader>> {
        self.map.lock().get(block_root).cloned()
    }
}
