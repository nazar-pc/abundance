use ab_client_database::storage_backend::{
    ClientDatabaseStorageBackend, OwnedPages, SharedPages, StorageBackendError,
};
use ab_direct_io_file::{DirectIoError, DirectIoFile, IoBufError};
use futures::channel::oneshot;
use std::io;
use tracing::{Span, debug};
use triomphe::Arc;

fn storage_backend_error(error: DirectIoError) -> StorageBackendError {
    match error {
        DirectIoError::AllocationFailed(error) => StorageBackendError::AllocationFailed(error),
        DirectIoError::Io(error) => StorageBackendError::Io(error),
    }
}

// TODO: This is a simple wrapper, but it will need to deal with multiple dynamic chains eventually
#[derive(Debug)]
pub(crate) struct FileStorageBackend {
    // TODO: Is wrapping with `Arc` actually necessary?
    file: Arc<DirectIoFile>,
    num_pages: u32,
}

impl ClientDatabaseStorageBackend for FileStorageBackend {
    #[inline(always)]
    fn num_pages(&self) -> u32 {
        self.num_pages
    }

    #[inline(always)]
    fn read(
        &self,
        buffer: OwnedPages,
        num_pages: u32,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, OwnedPages)> {
        let (sender, receiver) = oneshot::channel();

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                let result = match file.read_pages_at(buffer, num_pages, u64::from(offset)) {
                    Ok(buffer) => (Ok(()), buffer),
                    Err(IoBufError { error, buffer }) => {
                        (Err(storage_backend_error(error)), buffer)
                    }
                };

                if sender.send(result).is_err() {
                    debug!("Failed to send a read result back, receiver dropped");
                }
            }
        });

        receiver
    }

    #[inline(always)]
    fn write(
        &self,
        buffer: SharedPages,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, SharedPages)> {
        debug_assert!(
            buffer.len().is_multiple_of(SharedPages::ALIGN),
            "Buffer must contain whole pages"
        );

        let offset = u64::from(offset) * u64::from(SharedPages::ALIGN);
        let (sender, receiver) = oneshot::channel();

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                let result = match file.write_at(buffer, offset) {
                    Ok(buffer) => (Ok(()), buffer),
                    Err(IoBufError { error, buffer }) => {
                        (Err(storage_backend_error(error)), buffer)
                    }
                };

                if sender.send(result).is_err() {
                    debug!("Failed to send a write result back, receiver dropped");
                }
            }
        });

        receiver
    }
}

impl FileStorageBackend {
    #[inline(always)]
    pub(crate) fn new(file: Arc<DirectIoFile>) -> io::Result<Self> {
        // TODO: Support even larger databases, which might be needed/helpful for multiple chains
        //  and/or caching purposes
        let num_pages = u32::try_from(file.len()? / u64::from(OwnedPages::ALIGN))
            .map_err(|_error| io::Error::other("Database is too large"))?;

        Ok(Self { file, num_pages })
    }
}
