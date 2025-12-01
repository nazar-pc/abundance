pub(crate) mod page_group_header;
pub(crate) mod storage_item;

use crate::page_group::block::StorageItemBlock;
use crate::page_group::permanent::StorageItemPermanent;
use crate::storage_backend::{AlignedPage, ClientDatabaseStorageBackend};
use crate::storage_backend_adapter::storage_item::{
    StorageItem, StorageItemContainer, UniqueStorageItem,
};
use crate::{
    ClientDatabaseError, ClientDatabaseFormatError, ClientDatabaseFormatOptions, DatabaseId,
};
use ab_io_type::trivial_type::TrivialType;
use enum_map::{EnumMap, enum_map};
use futures::FutureExt;
use futures::channel::oneshot;
use page_group_header::StorageItemPageGroupHeader;
use rand::TryRngCore;
use rand::rngs::OsRng;
use replace_with::replace_with_or_abort_and_return;
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::task::Poll;
use std::{future, io};
use strum::FromRepr;
use tracing::{Instrument, debug, error, info_span};

#[derive(Debug, Copy, Clone, TrivialType, enum_map::Enum, FromRepr)]
#[repr(u8)]
pub(crate) enum PageGroupKind {
    /// These pages are stored permanently and are never removed
    Permanent = 0,
    /// These pages are related to blocks and expire over time as blocks become buried deeper in
    /// blockchain history
    Block = 1,
}

#[derive(Debug)]
struct PageGroup {
    /// Sequence number of the first storage item in this page group
    first_sequence_number: u64,
    /// Next page offset within the page group
    inner_next_page_offset: u32,
    /// Offset of the first page of this page group in the storage backend
    first_page_offset: u32,
}

#[derive(Debug)]
struct PageGroups {
    /// Next sequence number to use
    next_sequence_number: u64,
    /// A list of page groups.
    ///
    /// The front page is the active one, meaning it is being appended to, the back page is the
    /// oldest page.
    ///
    /// If pruning is needed, old pages are freed from back to front without gaps.
    list: VecDeque<PageGroup>,
}

#[derive(Debug)]
enum WriteBufferEntry {
    Free(Vec<AlignedPage>),
    Occupied(oneshot::Receiver<io::Result<Vec<AlignedPage>>>),
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct WriteLocation {
    #[expect(dead_code, reason = "Not used yet")]
    pub(crate) page_offset: u32,
    #[expect(dead_code, reason = "Not used yet")]
    pub(crate) num_pages: u32,
}

#[derive(Debug)]
pub(crate) struct StorageItemHandlerArg<SI> {
    pub(crate) storage_item: SI,
    pub(crate) page_offset: u32,
    pub(crate) num_pages: u32,
}

/// Storage item handlers are called on every storage item, storage items are read in the same order
/// they are defined in this data structure
#[derive(Debug)]
pub(crate) struct StorageItemHandlers<P, B> {
    /// Handler for storage items in permanent storage groups
    pub(crate) permanent: P,
    /// Handler for storage items in permanent block groups
    pub(crate) block: B,
}

#[derive(Debug)]
pub(crate) struct StorageBackendAdapter<StorageBackend> {
    database_id: DatabaseId,
    database_version: u8,
    /// Page group size in pages
    page_group_size: u32,
    storage_backend: StorageBackend,
    write_buffer: Box<[WriteBufferEntry]>,
    page_groups: EnumMap<PageGroupKind, PageGroups>,
    /// Offsets of the first pages that correspond to free page groups.
    ///
    /// Newly freed pages are added to the back, the oldest freed pages are pulled from the front.
    free_page_groups: VecDeque<u32>,
    had_write_failure: bool,
}

impl<StorageBackend> StorageBackendAdapter<StorageBackend>
where
    StorageBackend: ClientDatabaseStorageBackend,
{
    /// Current database version
    const VERSION: u8 = 0;

    pub(crate) async fn open<SIHP, SIHB>(
        write_buffer_size: usize,
        mut storage_item_handlers: StorageItemHandlers<SIHP, SIHB>,
        storage_backend: StorageBackend,
    ) -> Result<Self, ClientDatabaseError>
    where
        SIHP: FnMut(StorageItemHandlerArg<StorageItemPermanent>) -> Result<(), ClientDatabaseError>,
        SIHB: FnMut(StorageItemHandlerArg<StorageItemBlock>) -> Result<(), ClientDatabaseError>,
    {
        let database_id;
        let database_version;
        let page_group_size;
        let num_page_groups;

        let mut page_groups = enum_map! {
            PageGroupKind::Permanent => PageGroups {
                next_sequence_number: 0,
                list: VecDeque::new(),
            },
            PageGroupKind::Block => PageGroups {
                next_sequence_number: 0,
                list: VecDeque::new(),
            },
        };
        let mut free_page_groups = VecDeque::new();

        let mut buffer = Vec::new();

        // Check the first page group. This could have been done in the loop below, but that makes
        // the code even more ugly than this copy-paste.
        {
            buffer = storage_backend
                .read(buffer, 1, 0)
                .await
                .map_err(|_error| ClientDatabaseError::ReadRequestCancelled)?
                .map_err(|error| ClientDatabaseError::ReadError { error })?;

            let container =
                match StorageItemContainer::<StorageItemPageGroupHeader>::read_from_pages(&buffer) {
                    Ok(container) => container,
                    Err(_error) => {
                        // Page group header fit the first page, so any deciding error indicates it is
                        // not a valid page group header
                        return Err(ClientDatabaseError::Unformatted);
                    }
                };

            let page_group_header = &container.storage_item;
            if page_group_header.database_version != Self::VERSION {
                return Err(ClientDatabaseError::UnsupportedDatabaseVersion {
                    database_version: page_group_header.database_version,
                });
            }
            database_id = page_group_header.database_id;
            database_version = page_group_header.database_version;
            page_group_size = page_group_header.page_group_size;
            if page_group_size < 2 {
                return Err(ClientDatabaseError::PageGroupSizeTooSmall { page_group_size });
            }
            num_page_groups = storage_backend.num_pages() / page_group_size;

            match page_group_header.page_group_kind {
                PageGroupKind::Permanent => {
                    page_groups[PageGroupKind::Permanent]
                        .list
                        .push_front(PageGroup {
                            first_sequence_number: container.sequence_number,
                            inner_next_page_offset: container.num_pages(),
                            first_page_offset: 0,
                        });
                }
                PageGroupKind::Block => {
                    return Err(ClientDatabaseError::NonPermanentFirstPageGroup);
                }
            }
        }

        // Quick scan through the rest of page groups
        for page_group_index in 1..num_page_groups {
            let first_page_offset = page_group_index * page_group_size;
            buffer.clear();
            buffer = storage_backend
                .read(buffer, 1, first_page_offset)
                .await
                .map_err(|_error| ClientDatabaseError::ReadRequestCancelled)?
                .map_err(|error| ClientDatabaseError::ReadError { error })?;

            let container =
                match StorageItemContainer::<StorageItemPageGroupHeader>::read_from_pages(&buffer) {
                    Ok(container) => container,
                    Err(_error) => {
                        free_page_groups.push_back(first_page_offset);
                        continue;
                    }
                };

            let page_group_header = &container.storage_item;
            if !(page_group_header.database_id == database_id
                && page_group_header.database_version == database_version
                && page_group_header.page_group_size == page_group_size)
            {
                free_page_groups.push_back(first_page_offset);
                continue;
            }

            let page_group = PageGroup {
                first_sequence_number: container.sequence_number,
                inner_next_page_offset: container.num_pages(),
                first_page_offset,
            };
            page_groups[page_group_header.page_group_kind]
                .list
                .push_front(page_group);
        }

        // Sort page groups into the correct order of first sequence numbers
        for entry in page_groups.values_mut() {
            entry
                .list
                .make_contiguous()
                .sort_by_key(|page_group| Reverse(page_group.first_sequence_number));
        }

        // Read all permanent storage groups
        buffer = StorageBackendAdapter::read_page_groups::<StorageItemPermanent, _>(
            &mut page_groups[PageGroupKind::Permanent],
            page_group_size,
            &storage_backend,
            buffer,
            |container, page_offset| {
                let num_pages = container.num_pages();

                (storage_item_handlers.permanent)(StorageItemHandlerArg {
                    storage_item: container.storage_item,
                    page_offset,
                    num_pages,
                })
            },
        )
        .instrument(info_span!("", page_group_kind = ?PageGroupKind::Permanent))
        .await?;

        // Read all block storage groups
        let _ = StorageBackendAdapter::read_page_groups(
            &mut page_groups[PageGroupKind::Block],
            page_group_size,
            &storage_backend,
            buffer,
            |container, page_offset| {
                let num_pages = container.num_pages();

                (storage_item_handlers.block)(StorageItemHandlerArg {
                    storage_item: container.storage_item,
                    page_offset,
                    num_pages,
                })
            },
        )
        .instrument(info_span!("", page_group_kind = ?PageGroupKind::Block))
        .await?;

        Ok(Self {
            database_id,
            database_version,
            page_group_size,
            storage_backend,
            write_buffer: (0..write_buffer_size)
                .map(|_| WriteBufferEntry::Free(Vec::new()))
                .collect(),
            page_groups,
            free_page_groups,
            had_write_failure: false,
        })
    }

    pub(crate) async fn format(
        storage_backend: &StorageBackend,
        options: ClientDatabaseFormatOptions,
    ) -> Result<(), ClientDatabaseFormatError> {
        let mut buffer = Vec::with_capacity(1);

        if !options.force {
            buffer = storage_backend
                .read(buffer, 1, 0)
                .await
                .map_err(|_error| ClientDatabaseFormatError::ReadRequestCancelled)?
                .map_err(|error| ClientDatabaseFormatError::ReadError { error })?;
            buffer.clear();

            if StorageItemContainer::<StorageItemPageGroupHeader>::read_from_pages(&buffer).is_ok()
            {
                return Err(ClientDatabaseFormatError::AlreadyFormatted);
            }
        }

        let container = StorageItemContainer {
            sequence_number: 0,
            storage_item: StorageItemPageGroupHeader {
                database_id: DatabaseId::new({
                    let mut id = [0; 32];
                    OsRng.try_fill_bytes(&mut id)?;
                    id
                }),
                database_version: Self::VERSION,
                page_group_kind: PageGroupKind::Permanent,
                padding: [0; _],
                page_group_size: options.page_group_size.get(),
            },
        };
        Self::write_pages_to_buffer(&container, None, &mut buffer, 0)?;

        let _buffer: Vec<AlignedPage> = storage_backend
            .write(buffer, 0)
            .await
            .map_err(|_cancelled| ClientDatabaseFormatError::WriteRequestCancelled)??;

        Ok(())
    }

    /// Read all page groups and call the storage item handler for every storage item except the
    /// page group header
    async fn read_page_groups<SI, SIH>(
        target_page_groups: &mut PageGroups,
        page_group_size: u32,
        storage_backend: &StorageBackend,
        mut buffer: Vec<AlignedPage>,
        mut storage_item_handler: SIH,
    ) -> Result<Vec<AlignedPage>, ClientDatabaseError>
    where
        SI: StorageItem,
        SIH: FnMut(StorageItemContainer<SI>, u32) -> Result<(), ClientDatabaseError>,
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
                let container = match StorageItemContainer::read_from_pages(pages) {
                    Ok(container) => container,
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

                let sequence_number = container.sequence_number;
                let num_pages = container.num_pages();

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

                storage_item_handler(container, page_offset)?;

                pages = &pages[num_pages as usize..];
                page_group.inner_next_page_offset += num_pages;
            }
        }

        target_page_groups.next_sequence_number = next_sequence_number;

        Ok(buffer)
    }

    pub(super) async fn write_storage_item<SI>(
        &mut self,
        storage_item: SI,
    ) -> io::Result<WriteLocation>
    where
        SI: UniqueStorageItem,
    {
        if self.had_write_failure {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Previous write operation failed, writes are not allowed until restart",
            ));
        }

        self.write_storage_item_inner(storage_item)
            .await
            .inspect_err(|_error| {
                self.had_write_failure = true;
            })
    }

    async fn write_storage_item_inner<SI>(&mut self, storage_item: SI) -> io::Result<WriteLocation>
    where
        SI: UniqueStorageItem,
    {
        let page_group_kind = SI::page_group_kind();
        let target_page_groups = &mut self.page_groups[page_group_kind];

        let sequence_number = target_page_groups.next_sequence_number;
        target_page_groups.next_sequence_number += 1;

        let mut container = StorageItemContainer {
            sequence_number,
            storage_item,
        };

        let mut num_pages_to_write = container.num_pages();
        // Ensure a storage item doesn't exceed page group size. `-1` accounts for the page group
        // header.
        if num_pages_to_write > (self.page_group_size - 1) {
            return Err(io::Error::new(
                io::ErrorKind::QuotaExceeded,
                format!(
                    "Storage item is too large: {num_pages_to_write} pages, max supported is {} \
                    pages",
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
            && remaining_pages_in_group >= num_pages_to_write
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

            let page_group_header = StorageItemContainer {
                sequence_number,
                storage_item: StorageItemPageGroupHeader {
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
            container.sequence_number += 1;
            // Add a page that corresponds to the page group header
            num_pages_to_write += 1;

            let active_page_group = target_page_groups.list.push_front_mut(PageGroup {
                first_sequence_number: sequence_number,
                inner_next_page_offset: 0,
                first_page_offset,
            });

            (active_page_group, Some(page_group_header))
        };

        let write_page_offset =
            active_page_group.first_page_offset + active_page_group.inner_next_page_offset;
        active_page_group.inner_next_page_offset += num_pages_to_write;

        // In case buffering is disabled, allocate a buffer on demand and wait for write to
        // finish
        if self.write_buffer.is_empty() {
            let mut buffer = Vec::new();

            let page_offset = Self::write_pages_to_buffer(
                &container,
                maybe_page_group_header.as_ref(),
                &mut buffer,
                write_page_offset,
            )?;

            let _buffer: Vec<_> = self
                .storage_backend
                .write(buffer, write_page_offset)
                .await
                .map_err(|_cancelled| {
                    io::Error::new(
                        io::ErrorKind::Interrupted,
                        "Storage backend write was aborted",
                    )
                })
                .flatten()?;

            return Ok(WriteLocation {
                page_offset,
                num_pages: container.num_pages(),
            });
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
                                    Ok(mut buffer) => {
                                        buffer.clear();

                                        buffer
                                    }
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

                    // Write storage item pages to the buffer
                    let page_offset = match Self::write_pages_to_buffer(
                        &container,
                        maybe_page_group_header.as_ref(),
                        &mut buffer,
                        write_page_offset,
                    ) {
                        Ok(page_offset) => page_offset,
                        Err(error) => {
                            buffer.clear();
                            return (
                                Some(Err(io::Error::other(error))),
                                WriteBufferEntry::Free(buffer),
                            );
                        }
                    };

                    let receiver = self.storage_backend.write(buffer, write_page_offset);
                    (
                        Some(Ok(WriteLocation {
                            page_offset,
                            num_pages: container.num_pages(),
                        })),
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

    /// Write (append) a storage item with an optional page group header in front of it.
    ///
    /// Returns page offset at which an item is written based on `preliminary_page_offset`.
    #[inline(always)]
    fn write_pages_to_buffer<SI>(
        container: &StorageItemContainer<SI>,
        maybe_page_group_header: Option<&StorageItemContainer<StorageItemPageGroupHeader>>,
        buffer: &mut Vec<AlignedPage>,
        preliminary_page_offset: u32,
    ) -> io::Result<u32>
    where
        SI: StorageItem,
    {
        if let Some(page_group_header) = maybe_page_group_header {
            let length = container.num_pages() as usize + 1;
            buffer.reserve(length);

            {
                let (header, buffer) = buffer.spare_capacity_mut()[..length].split_at_mut(1);
                page_group_header
                    .write_to_pages(header)
                    .map_err(io::Error::other)?;
                container.write_to_pages(buffer).map_err(io::Error::other)?;
            }
            // SAFETY: Successful writes above fully initialized `length` pages
            unsafe {
                buffer.set_len(buffer.len() + length);
            }

            // +1 because the page header was written in front of the storage item
            Ok(preliminary_page_offset + 1)
        } else {
            let length = container.num_pages() as usize;
            buffer.reserve(length);

            container
                .write_to_pages(&mut buffer.spare_capacity_mut()[..length])
                .map_err(io::Error::other)?;
            // SAFETY: Successful write above fully initialized `length` pages
            unsafe {
                buffer.set_len(buffer.len() + length);
            }

            Ok(preliminary_page_offset)
        }
    }
}
