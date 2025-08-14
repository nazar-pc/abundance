use ab_client_database::storage_backend::{AlignedPage, ClientDatabaseStorageBackend};
use ab_direct_io_file::DirectIoFile;
use futures::channel::oneshot;
use rclite::Arc;
use std::io;
use tracing::{Span, debug};

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
        mut buffer: Vec<AlignedPage>,
        length: u32,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>> {
        let (sender, receiver) = oneshot::channel();

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                buffer.reserve(length as usize);

                let bytes = AlignedPage::uninit_slice_mut_to_repr(
                    &mut buffer.spare_capacity_mut()[..length as usize],
                );
                let bytes = ab_direct_io_file::AlignedPage::try_uninit_slice_mut_from_repr(bytes)
                    .expect("Correctly aligned as it comes from another aligned buffer type; qed");
                let result = match file.read_exact_at_raw(bytes, offset as u64) {
                    Ok(()) => {
                        // SAFETY: Just written `length` bytes
                        unsafe {
                            let new_len = buffer.len() + length as usize;
                            buffer.set_len(new_len);
                        }
                        Ok(buffer)
                    }
                    Err(error) => Err(error),
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
        buffer: Vec<AlignedPage>,
        offset: u32,
    ) -> oneshot::Receiver<io::Result<Vec<AlignedPage>>> {
        let (sender, receiver) = oneshot::channel();

        tokio::task::spawn_blocking({
            let file = Arc::clone(&self.file);
            let span = Span::current();

            move || {
                let _guard = span.enter();

                let bytes = AlignedPage::slice_to_repr(&buffer);
                let bytes = ab_direct_io_file::AlignedPage::try_slice_from_repr(bytes)
                    .expect("Correctly aligned as it comes from another aligned buffer type; qed");
                let result = match file.write_all_at_raw(bytes, offset as u64) {
                    Ok(()) => Ok(buffer),
                    Err(error) => Err(error),
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
        let num_pages = u32::try_from(file.len()? / AlignedPage::SIZE as u64)
            .map_err(|_error| io::Error::other("Database is too large"))?;

        Ok(Self { file, num_pages })
    }
}
