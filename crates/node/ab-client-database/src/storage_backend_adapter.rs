use crate::storage_backend::{AlignedPage, ClientDatabaseStorageBackend};
use crate::storage_item::{StorageItem, StorageItemKind};
use futures::FutureExt;
use futures::channel::oneshot;
use replace_with::replace_with_or_abort_and_return;
use std::collections::VecDeque;
use std::task::Poll;
use std::{future, io};

#[derive(Debug)]
pub(crate) struct PageGroup {
    pub(crate) first_sequence_number: u64,
    /// Next page offset within the page group
    pub(crate) inner_next_page_offset: u32,
    /// Offset of the first page of this page group in the storage backend
    pub(crate) first_page_offset: u32,
}

#[derive(Debug)]
pub(crate) struct PageGroups {
    /// Permanent page groups.
    ///
    /// The last page group is the active one, meaning it is being appended to.
    pub(crate) permanent: Vec<PageGroup>,
    /// Ephemeral page groups.
    ///
    /// The front page is the active one, meaning it is being appended to, the back page is the
    /// oldest page. Old pages are freed from back to front without gaps.
    pub(crate) ephemeral: VecDeque<PageGroup>,
    /// Offsets of the first pages that correspond to free page groups.
    ///
    /// Newly freed pages are added to the back, the oldest freed pages are pulled from the front.
    pub(crate) free: VecDeque<u32>,
}

#[derive(Debug)]
pub(crate) enum WriteBufferEntry {
    Free(Vec<AlignedPage>),
    Occupied(oneshot::Receiver<io::Result<Vec<AlignedPage>>>),
}

#[derive(Debug)]
pub(crate) struct WriteLocation {
    pub(crate) page_offset: u32,
}

#[derive(Debug)]
pub(crate) struct StorageBackendAdapter {
    /// Page group size in pages
    page_group_size: u32,
    next_permanent_sequence_number: u64,
    next_ephemeral_sequence_number: u64,
    write_buffer: Box<[WriteBufferEntry]>,
    page_groups: PageGroups,
}

impl StorageBackendAdapter {
    pub(crate) fn new(
        page_group_size: u32,
        next_permanent_sequence_number: u64,
        next_ephemeral_sequence_number: u64,
        write_buffer: Box<[WriteBufferEntry]>,
        page_groups: PageGroups,
    ) -> Self {
        Self {
            page_group_size,
            next_permanent_sequence_number,
            next_ephemeral_sequence_number,
            write_buffer,
            page_groups,
        }
    }

    // TODO: Consider making write errors permanent, so all future writes fail after the first
    //  failure
    pub(super) async fn write_ephemeral_storage_item<StorageBackend>(
        &mut self,
        storage_backend: &StorageBackend,
        storage_item_kind: StorageItemKind,
    ) -> io::Result<WriteLocation>
    where
        StorageBackend: ClientDatabaseStorageBackend,
    {
        let sequence_number = self.next_ephemeral_sequence_number;
        self.next_ephemeral_sequence_number += 1;

        let storage_item = StorageItem {
            sequence_number,
            kind: storage_item_kind,
        };

        let num_pages = storage_item.num_pages();
        // Ensure a storage item doesn't exceed page group size. `-1` accounts for the page group
        // header.
        if num_pages > (self.page_group_size - 1) {
            return Err(io::Error::new(
                io::ErrorKind::QuotaExceeded,
                format!(
                    "Storage item is too large: {} pages, max supported is {} pages",
                    num_pages, self.page_group_size
                ),
            ));
        }

        // Check if there is an active page group and whether it has enough free pages in it
        let active_page_group = if let Some(page_group) = self.page_groups.ephemeral.front_mut()
            && let Some(remaining_pages_in_group) = self
                .page_group_size
                .checked_sub(page_group.inner_next_page_offset)
            && remaining_pages_in_group >= num_pages
        {
            page_group
        } else {
            // Allocate a new page group
            let first_page_offset = self.page_groups.free.pop_front().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::StorageFull,
                    "No free pages available to write a new storage item",
                )
            })?;

            // TODO: Write page group header

            self.page_groups.ephemeral.push_front(PageGroup {
                first_sequence_number: sequence_number,
                inner_next_page_offset: 0,
                first_page_offset,
            });
            self.page_groups
                .ephemeral
                .front_mut()
                .expect("Just inserted; qed")
        };

        let page_offset =
            active_page_group.first_page_offset + active_page_group.inner_next_page_offset;
        active_page_group.inner_next_page_offset -= num_pages;

        if self.write_buffer.is_empty() {
            // In case buffering is disabled, allocate buffer on demand and wait for write to finish
            let mut buffer = vec![AlignedPage::default(); num_pages as usize];
            storage_item
                .write_to_pages(&mut buffer)
                .map_err(io::Error::other)?;

            let _buffer: Vec<_> = storage_backend
                .write(buffer, page_offset)
                .await
                .map_err(|_cancelled| {
                    io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Storage backend write was aborted",
                    )
                })
                .flatten()?;

            return Ok(WriteLocation { page_offset });
        }

        let write_fut = future::poll_fn(|cx| {
            // Find a free write buffer entry among those that are either completely free or already
            // finished and can be reused
            let write_attempt_result = self.write_buffer.iter_mut().find_map(|entry| {
                replace_with_or_abort_and_return(entry, |entry| {
                    let mut buffer = match entry {
                        // Already free buffer
                        WriteBufferEntry::Free(buffer) => buffer,
                        WriteBufferEntry::Occupied(mut receiver) => {
                            // Poll pending write attempt
                            match receiver.poll_unpin(cx) {
                                Poll::Ready(Ok(write_result)) => match write_result {
                                    // Write succeeded, reuse buffer
                                    Ok(buffer) => buffer,
                                    // Write failed
                                    Err(error) => {
                                        return (
                                            Some(Err(error)),
                                            WriteBufferEntry::Occupied(receiver),
                                        );
                                    }
                                },
                                // Write attempt was aborted
                                Poll::Ready(Err(_cancelled)) => {
                                    return (
                                        Some(Err(io::Error::new(
                                            io::ErrorKind::Interrupted,
                                            "Storage backend write was aborted",
                                        ))),
                                        WriteBufferEntry::Occupied(receiver),
                                    );
                                }
                                // Still in progress
                                Poll::Pending => {
                                    return (None, WriteBufferEntry::Occupied(receiver));
                                }
                            }
                        }
                    };

                    // Resize buffer and write storage item pages
                    buffer.resize_with(num_pages as usize, AlignedPage::default);
                    if let Err(error) = storage_item.write_to_pages(&mut buffer) {
                        return (
                            Some(Err(io::Error::other(error))),
                            WriteBufferEntry::Free(buffer),
                        );
                    }

                    let receiver = storage_backend.write(buffer, page_offset);
                    (
                        Some(Ok(WriteLocation { page_offset })),
                        WriteBufferEntry::Occupied(receiver),
                    )
                })
            });

            match write_attempt_result {
                Some(result) => Poll::Ready(result),
                None => Poll::Pending,
            }
        });

        write_fut.await
    }
}
