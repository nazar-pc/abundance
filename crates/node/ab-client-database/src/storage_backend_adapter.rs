use crate::storage_backend::{AlignedPage, ClientDatabaseStorageBackend};
use crate::storage_item::page_group_header::StorageItemPageGroupHeader;
use crate::storage_item::{StorageItem, StorageItemKind};
use crate::{ClientDatabaseError, DatabaseId, PageGroupKind};
use enum_map::EnumMap;
use futures::FutureExt;
use futures::channel::oneshot;
use replace_with::replace_with_or_abort_and_return;
use std::collections::VecDeque;
use std::task::Poll;
use std::{future, io};
use tracing::{debug, error};

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
    /// Next sequence number to use
    pub(crate) next_sequence_number: u64,
    /// A list of page groups.
    ///
    /// The front page is the active one, meaning it is being appended to, the back page is the
    /// oldest page.
    ///
    /// If pruning is needed, old pages are freed from back to front without gaps.
    pub(crate) list: VecDeque<PageGroup>,
}

#[derive(Debug)]
pub(crate) enum WriteBufferEntry {
    Free(Vec<AlignedPage>),
    Occupied(oneshot::Receiver<io::Result<Vec<AlignedPage>>>),
}

#[derive(Debug)]
pub(crate) struct WriteLocation {
    #[expect(dead_code, reason = "Not used yet")]
    pub(crate) page_offset: u32,
}

#[derive(Debug)]
pub(crate) struct StorageBackendAdapter {
    database_id: DatabaseId,
    database_version: u8,
    /// Page group size in pages
    page_group_size: u32,
    write_buffer: Box<[WriteBufferEntry]>,
    page_groups: EnumMap<PageGroupKind, PageGroups>,
    /// Offsets of the first pages that correspond to free page groups.
    ///
    /// Newly freed pages are added to the back, the oldest freed pages are pulled from the front.
    free_page_groups: VecDeque<u32>,
    had_write_failure: bool,
}

impl StorageBackendAdapter {
    pub(crate) fn new(
        database_id: DatabaseId,
        database_version: u8,
        page_group_size: u32,
        write_buffer: Box<[WriteBufferEntry]>,
        page_groups: EnumMap<PageGroupKind, PageGroups>,
        free_page_groups: VecDeque<u32>,
    ) -> Self {
        Self {
            database_id,
            database_version,
            page_group_size,
            write_buffer,
            page_groups,
            free_page_groups,
            had_write_failure: false,
        }
    }

    /// Read all page groups and call the storage item handler for every storage item except the
    /// page group header
    pub(crate) async fn read_page_groups<StorageBackend, Kind, SIH>(
        target_page_groups: &mut PageGroups,
        page_group_size: u32,
        storage_backend: &StorageBackend,
        mut buffer: Vec<AlignedPage>,
        mut storage_item_handler: SIH,
    ) -> Result<Vec<AlignedPage>, ClientDatabaseError>
    where
        StorageBackend: ClientDatabaseStorageBackend,
        Kind: StorageItemKind,
        SIH: FnMut(StorageItem<Kind>, u32) -> Result<(), ClientDatabaseError>,
    {
        let mut next_sequence_number = 0;

        // Read all page groups from oldest to newest
        for page_group in target_page_groups.list.iter_mut().rev() {
            if next_sequence_number == 0 {
                next_sequence_number = page_group.first_sequence_number;
            }

            buffer.clear();
            buffer = storage_backend
                .read(
                    buffer,
                    // Substraction accounts for the page group header, which was already read
                    page_group_size - page_group.inner_next_page_offset,
                    page_group.first_page_offset + page_group.inner_next_page_offset,
                )
                .await
                .map_err(|_error| ClientDatabaseError::ReadRequestCancelled)?
                .map_err(|error| ClientDatabaseError::ReadError { error })?;

            // Account for the page group header that was already read
            if page_group.first_sequence_number == next_sequence_number {
                next_sequence_number += 1;
            } else {
                error!(
                    actual = page_group.first_sequence_number,
                    expected = next_sequence_number,
                    "Unexpected first sequence number"
                );
                return Err(ClientDatabaseError::UnexpectedSequenceNumber {
                    actual: page_group.first_sequence_number,
                    expected: next_sequence_number,
                    page_offset: page_group.first_page_offset,
                });
            }

            let mut pages = buffer.as_slice();

            while !pages.is_empty() {
                let page_offset = page_group.first_page_offset + page_group.inner_next_page_offset;
                let storage_item = match StorageItem::read_from_pages(pages) {
                    Ok(storage_item) => storage_item,
                    Err(error) => {
                        debug!(
                            page_offset,
                            %error,
                            "Failed to read storage item, considering this to be the end of the \
                            page group"
                        );
                        break;
                    }
                };

                let sequence_number = storage_item.sequence_number;
                let num_pages = storage_item.num_pages();

                if sequence_number == next_sequence_number {
                    next_sequence_number += 1;
                } else {
                    error!(
                        page_offset,
                        actual = sequence_number,
                        expected = next_sequence_number,
                        "Unexpected sequence number"
                    );
                    return Err(ClientDatabaseError::UnexpectedSequenceNumber {
                        actual: sequence_number,
                        expected: next_sequence_number,
                        page_offset,
                    });
                }

                storage_item_handler(storage_item, page_offset)?;

                pages = &pages[num_pages as usize..];
                page_group.inner_next_page_offset += num_pages;
            }
        }

        target_page_groups.next_sequence_number = next_sequence_number;

        Ok(buffer)
    }

    pub(super) async fn write_storage_item<StorageBackend, Kind>(
        &mut self,
        page_group_kind: PageGroupKind,
        storage_backend: &StorageBackend,
        storage_item_kind: Kind,
    ) -> io::Result<WriteLocation>
    where
        StorageBackend: ClientDatabaseStorageBackend,
        Kind: StorageItemKind,
    {
        if self.had_write_failure {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Previous write operation failed, writes are not allowed until restart",
            ));
        }

        self.write_storage_item_inner(page_group_kind, storage_backend, storage_item_kind)
            .await
            .inspect_err(|_error| {
                self.had_write_failure = true;
            })
    }

    async fn write_storage_item_inner<StorageBackend, Kind>(
        &mut self,
        page_group_kind: PageGroupKind,
        storage_backend: &StorageBackend,
        storage_item_kind: Kind,
    ) -> io::Result<WriteLocation>
    where
        StorageBackend: ClientDatabaseStorageBackend,
        Kind: StorageItemKind,
    {
        let target_page_groups = &mut self.page_groups[page_group_kind];

        let sequence_number = target_page_groups.next_sequence_number;
        target_page_groups.next_sequence_number += 1;

        let mut storage_item = StorageItem {
            sequence_number,
            storage_item_kind,
        };

        let mut num_pages = storage_item.num_pages();
        // Ensure a storage item doesn't exceed page group size. `-1` accounts for the page group
        // header.
        if num_pages > (self.page_group_size - 1) {
            return Err(io::Error::new(
                io::ErrorKind::QuotaExceeded,
                format!(
                    "Storage item is too large: {num_pages} pages, max supported is {} pages",
                    self.page_group_size
                ),
            ));
        }

        // Check if there is an active page group and whether it has enough free pages in it
        let (active_page_group, maybe_page_group_header) = if let Some(page_group) =
            target_page_groups.list.front_mut()
            && let Some(remaining_pages_in_group) = self
                .page_group_size
                .checked_sub(page_group.inner_next_page_offset)
            && remaining_pages_in_group >= num_pages
        {
            (page_group, None)
        } else {
            // Allocate a new page group
            let first_page_offset = self.free_page_groups.pop_front().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::StorageFull,
                    "No free pages available to write a new storage item",
                )
            })?;

            let page_group_header = StorageItem {
                sequence_number,
                storage_item_kind: StorageItemPageGroupHeader {
                    database_id: self.database_id,
                    database_version: self.database_version,
                    page_group_kind,
                    padding: [0; _],
                    page_group_size: self.page_group_size,
                },
            };

            // Adjust sequence numbers since the previous value was reused by a new page group
            // header
            target_page_groups.next_sequence_number += 1;
            storage_item.sequence_number += 1;
            // Add a page that corresponds to the page group header
            num_pages += 1;

            target_page_groups.list.push_front(PageGroup {
                first_sequence_number: sequence_number,
                inner_next_page_offset: 0,
                first_page_offset,
            });
            let active_page_group = target_page_groups
                .list
                .front_mut()
                .expect("Just inserted; qed");

            (active_page_group, Some(page_group_header))
        };

        let page_offset =
            active_page_group.first_page_offset + active_page_group.inner_next_page_offset;
        active_page_group.inner_next_page_offset += num_pages;

        // In case buffering is disabled, allocate a buffer on demand and wait for write to
        // finish
        if self.write_buffer.is_empty() {
            let mut buffer = Vec::new();

            Self::write_pages_to_buffer(
                &storage_item,
                maybe_page_group_header.as_ref(),
                &mut buffer,
            )?;

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
                    if let Err(error) = Self::write_pages_to_buffer(
                        &storage_item,
                        maybe_page_group_header.as_ref(),
                        &mut buffer,
                    ) {
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

    /// Resize the buffer to the correct size and write storage item with optional prepended page
    /// group header
    #[inline(always)]
    fn write_pages_to_buffer<Kind>(
        storage_item: &StorageItem<Kind>,
        maybe_page_group_header: Option<&StorageItem<StorageItemPageGroupHeader>>,
        buffer: &mut Vec<AlignedPage>,
    ) -> io::Result<()>
    where
        Kind: StorageItemKind,
    {
        if let Some(page_group_header) = maybe_page_group_header {
            buffer.resize_with(storage_item.num_pages() as usize + 1, AlignedPage::default);

            let (header, buffer) = buffer.split_at_mut(1);
            page_group_header
                .write_to_pages(header)
                .map_err(io::Error::other)?;
            storage_item
                .write_to_pages(buffer)
                .map_err(io::Error::other)?;
        } else {
            buffer.resize_with(storage_item.num_pages() as usize, AlignedPage::default);

            storage_item
                .write_to_pages(buffer)
                .map_err(io::Error::other)?;
        }

        Ok(())
    }
}
