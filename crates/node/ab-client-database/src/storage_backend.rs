use ab_aligned_buffer::alignment::Align4096;
use ab_aligned_buffer::{AllocationError, OwnedAlignedBuffer, SharedAlignedBuffer};
use futures::channel::oneshot;
use std::{fmt, io};

/// Owned buffer of pages, which are the unit of reads and writes of a storage backend.
///
/// The size of a page is the alignment of the buffer, [`OwnedPages::ALIGN`] bytes. 4096 bytes is a
/// relatively safe size due to sector size on SSDs commonly being 512 or 4096 bytes.
pub type OwnedPages = OwnedAlignedBuffer<Align4096>;
/// Shared buffer of pages, see [`OwnedPages`]
pub type SharedPages = SharedAlignedBuffer<Align4096>;

/// Error of a storage backend operation
#[derive(Debug, thiserror::Error)]
pub enum StorageBackendError {
    /// Failed to allocate a buffer for the requested pages
    #[error("Failed to allocate a buffer: {0}")]
    AllocationFailed(#[from] AllocationError),
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl From<StorageBackendError> for io::Error {
    #[inline]
    fn from(error: StorageBackendError) -> Self {
        match error {
            StorageBackendError::AllocationFailed(_) => {
                io::Error::new(io::ErrorKind::OutOfMemory, error)
            }
            StorageBackendError::Io(error) => error,
        }
    }
}

/// Storage backend to be used by [`ClientDatabase`].
///
/// Read and write errors are fatal: the database doesn't try to recover from them, and the node is
/// expected to stop when it encounters one.
///
/// [`ClientDatabase`]: crate::ClientDatabase
pub trait ClientDatabaseStorageBackend: fmt::Debug + Send + Sync + 'static {
    /// Total number of pages available for reads/writes
    fn num_pages(&self) -> u32;

    /// Read `num_pages` pages at `offset` (in pages) into `buffer`, replacing its contents.
    ///
    /// The buffer grows if its capacity is smaller than `num_pages` pages. Existing contents are
    /// discarded, so they are never copied, and [`StorageBackendError::AllocationFailed`] is
    /// returned if growing fails. On success the buffer contains exactly `num_pages` pages. The
    /// buffer is returned back regardless of the result.
    ///
    /// The read is started before this method returns.
    fn read(
        &self,
        buffer: OwnedPages,
        num_pages: u32,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, OwnedPages)>;

    /// Write the contents of `buffer`, which are whole pages, at `offset` (in pages).
    ///
    /// The buffer is shared, so the caller can keep reading it while the write is in progress. The
    /// buffer is returned back regardless of the result.
    ///
    /// The write is started before this method returns.
    fn write(
        &self,
        buffer: SharedPages,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, SharedPages)>;
}
