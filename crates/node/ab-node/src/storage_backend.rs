use ab_aligned_buffer::AllocationError;
use ab_client_database::storage_backend::{
    ClientDatabaseStorageBackend, OwnedPages, SharedPages, StorageBackendError,
};
use ab_direct_io_file::{AlignedPage, DirectIoFile};
use futures::channel::oneshot;
use std::io::BorrowedBuf;
use std::mem::MaybeUninit;
use std::{io, slice};
use tracing::{Span, debug};
use triomphe::Arc;

const {
    assert!(OwnedPages::ALIGN as usize == AlignedPage::SIZE);
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
        mut buffer: OwnedPages,
        num_pages: u32,
        offset: u32,
    ) -> oneshot::Receiver<(Result<(), StorageBackendError>, OwnedPages)> {
        let (sender, receiver) = oneshot::channel();

        // Content is replaced, so nothing is copied if the buffer has to grow
        buffer.truncate(0);
        let maybe_length = num_pages
            .checked_mul(OwnedPages::ALIGN)
            .ok_or(AllocationError::CapacityOverflow)
            .and_then(|length| buffer.try_ensure_capacity(length).map(|()| length));
        let length = match maybe_length {
            Ok(length) => length,
            Err(error) => {
                if sender.send((Err(error.into()), buffer)).is_err() {
                    debug!("Failed to send a read result back, receiver dropped");
                }
                return receiver;
            }
        };
        let offset = u64::from(offset) * u64::from(OwnedPages::ALIGN);

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                // TODO: Remove the conversion once direct I/O file supports aligned buffers
                let bytes = &mut buffer.spare_capacity_mut()[..length as usize];
                // SAFETY: `MaybeUninit<[u8; N]>` has the same layout as `[MaybeUninit<u8>; N]`, and
                // the length is a multiple of the page size, which `AlignedPage::SIZE` is
                let pages = unsafe {
                    slice::from_raw_parts_mut(
                        bytes
                            .as_mut_ptr()
                            .cast::<MaybeUninit<[u8; AlignedPage::SIZE]>>(),
                        bytes.len() / AlignedPage::SIZE,
                    )
                };
                let pages = AlignedPage::try_uninit_slice_mut_from_repr(pages)
                    .expect("Contents of the buffer are aligned to the page size; qed");
                let result = file
                    .read_exact_at_raw(BorrowedBuf::from(pages).unfilled(), offset)
                    .map_err(StorageBackendError::from);
                if result.is_ok() {
                    // SAFETY: Just read `length` bytes
                    unsafe {
                        buffer.set_len(length);
                    }
                }

                if sender.send((result, buffer)).is_err() {
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
        let offset = u64::from(offset) * u64::from(SharedPages::ALIGN);
        let (sender, receiver) = oneshot::channel();

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                // TODO: Remove the conversion once direct I/O file supports aligned buffers
                let (pages, remainder) = buffer.as_chunks::<{ AlignedPage::SIZE }>();
                let result = if remainder.is_empty() {
                    let pages = AlignedPage::try_slice_from_repr(pages)
                        .expect("Contents of the buffer are aligned to the page size; qed");
                    file.write_all_at_raw(pages, offset)
                        .map_err(StorageBackendError::from)
                } else {
                    Err(StorageBackendError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Buffer doesn't contain whole pages",
                    )))
                };

                if sender.send((result, buffer)).is_err() {
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
