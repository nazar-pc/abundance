//! Cross-platform APIs for working with files using direct I/O.
//!
//! Depending on OS, this will use direct I/O, unbuffered, uncaches passthrough file reads/writes,
//! bypassing as much of OS machinery as possible.
//!
//! NOTE: There are major alignment requirements described here:
//! <https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering#alignment-and-file-access-requirements>
//! <https://man7.org/linux/man-pages/man2/open.2.html>

#![feature(core_io_borrowed_buf, maybe_uninit_as_bytes)]
#![cfg_attr(unix, feature(read_buf_at))]
#![cfg_attr(windows, feature(seek_read_exact_seek_write_all))]
// TODO: Remove once https://github.com/rust-lang/rust-clippy/issues/17498 is resolved
#![cfg_attr(
    any(unix, windows),
    expect(
        clippy::items_after_statements,
        reason = "https://github.com/rust-lang/rust-clippy/issues/17498"
    )
)]

// TODO: Windows shims are incomplete under Miri: https://github.com/rust-lang/miri/issues/3482
#[cfg(all(test, not(all(miri, windows))))]
mod tests;

use ab_aligned_buffer::alignment::{Align1, Align4096};
use ab_aligned_buffer::{AlignedBuffer, AllocationError, OwnedAlignedBuffer, SharedAlignedBuffer};
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BorrowedBuf, BorrowedCursor};
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::path::Path;
use std::{io, mem};

/// Page of memory aligned to its size, used for read-modify-write of partially written pages
#[repr(C, align(4096))]
struct Page([u8; OwnedPages::ALIGN as usize]);

const _: () = {
    assert!(align_of::<Page>() == OwnedPages::ALIGN as usize);
};

/// Owned buffer of pages, which are the unit of direct I/O.
///
/// The size of a page is the alignment of the buffer, [`OwnedPages::ALIGN`] bytes. 4096 bytes is a
/// relatively safe size due to sector size on SSDs commonly being 512 or 4096 bytes.
pub type OwnedPages = OwnedAlignedBuffer<Align4096>;
/// Shared buffer of pages, see [`OwnedPages`]
pub type SharedPages = SharedAlignedBuffer<Align4096>;

/// Error of a direct I/O operation
#[derive(Debug, thiserror::Error)]
pub enum DirectIoError {
    /// Failed to allocate a buffer
    #[error("Failed to allocate a buffer: {0}")]
    AllocationFailed(#[from] AllocationError),
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl From<DirectIoError> for io::Error {
    #[inline]
    fn from(error: DirectIoError) -> Self {
        match error {
            DirectIoError::AllocationFailed(_) => io::Error::new(io::ErrorKind::OutOfMemory, error),
            DirectIoError::Io(error) => error,
        }
    }
}

/// Error of an operation that takes a buffer by value, which is returned back, so that its
/// allocation can be reused
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct IoBufError<B> {
    /// Low-level error
    pub error: DirectIoError,
    /// Buffer that was given to the operation
    pub buffer: B,
}

impl<B> From<IoBufError<B>> for io::Error {
    #[inline]
    fn from(error: IoBufError<B>) -> Self {
        error.error.into()
    }
}

/// Restrict how much data to read from the disk in a single call to avoid very large memory usage
const MAX_READ_SIZE: usize = 1024 * 1024;

const _: () = {
    assert!(MAX_READ_SIZE.is_multiple_of(AlignedPage::SIZE));
};

/// A wrapper data structure with 4096 bytes alignment, which is the most common alignment for
/// direct I/O operations.
#[derive(Debug, Copy, Clone)]
#[repr(C, align(4096))]
pub struct AlignedPage([u8; AlignedPage::SIZE]);

const _: () = {
    assert!(align_of::<AlignedPage>() == AlignedPage::SIZE);
};

impl Default for AlignedPage {
    #[inline(always)]
    fn default() -> Self {
        Self([0; AlignedPage::SIZE])
    }
}

impl AlignedPage {
    /// 4096 is as a relatively safe size due to sector size on SSDs commonly being 512 or 4096
    /// bytes
    pub const SIZE: usize = 4096;

    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline(always)]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; AlignedPage::SIZE]] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` with a single `[u8; AlignedPage::SIZE]` field and
        // alignment equal to its size, so it has no padding and the same size as the inner value,
        // whose alignment is smaller
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline(always)]
    pub fn uninit_slice_to_repr(
        value: &[MaybeUninit<Self>],
    ) -> &[MaybeUninit<[u8; AlignedPage::SIZE]>] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` with a single `[u8; AlignedPage::SIZE]` field and
        // alignment equal to its size, so it has no padding and the same size as the inner value,
        // whose alignment is smaller. `MaybeUninit` preserves the size and alignment of the wrapped
        // type.
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_from_repr(value: &[[u8; AlignedPage::SIZE]]) -> Option<&[Self]> {
        // SAFETY: All bit patterns are valid and there is no padding
        let (before, slice, after) = unsafe { value.align_to::<Self>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_uninit_slice_from_repr(
        value: &[MaybeUninit<[u8; AlignedPage::SIZE]>],
    ) -> Option<&[MaybeUninit<Self>]> {
        // SAFETY: All bit patterns are valid and there is no padding
        let (before, slice, after) = unsafe { value.align_to::<MaybeUninit<Self>>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline(always)]
    pub fn slice_mut_to_repr(slice: &mut [Self]) -> &mut [[u8; AlignedPage::SIZE]] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` with a single `[u8; AlignedPage::SIZE]` field and
        // alignment equal to its size, so it has no padding and the same size as the inner value,
        // whose alignment is smaller. Any bytes written through the result are valid `AlignedPage`.
        unsafe { mem::transmute(slice) }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline(always)]
    pub fn uninit_slice_mut_to_repr(
        slice: &mut [MaybeUninit<Self>],
    ) -> &mut [MaybeUninit<[u8; AlignedPage::SIZE]>] {
        // SAFETY: `AlignedPage` is `#[repr(C)]` with a single `[u8; AlignedPage::SIZE]` field and
        // alignment equal to its size, so it has no padding and the same size as the inner value,
        // whose alignment is smaller. `MaybeUninit` preserves the size and alignment of the wrapped
        // type, and any bytes (initialized or not) written through the result are a valid
        // `MaybeUninit<AlignedPage>`.
        unsafe { mem::transmute(slice) }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_mut_from_repr(value: &mut [[u8; AlignedPage::SIZE]]) -> Option<&mut [Self]> {
        // SAFETY: All bit patterns are valid and there is no padding
        let (before, slice, after) = unsafe { value.align_to_mut::<Self>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from a slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_uninit_slice_mut_from_repr(
        value: &mut [MaybeUninit<[u8; AlignedPage::SIZE]>],
    ) -> Option<&mut [MaybeUninit<Self>]> {
        // SAFETY: All bit patterns are valid and there is no padding
        let (before, slice, after) = unsafe { value.align_to_mut::<MaybeUninit<Self>>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }
}

/// Wrapper data structure for direct/unbuffered/uncached I/O.
///
/// Depending on OS, this will use direct I/O, unbuffered, uncaches passthrough file reads/writes,
/// bypassing as much of OS machinery as possible.
///
/// NOTE: There are major alignment requirements described here:
/// <https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering#alignment-and-file-access-requirements>
/// <https://man7.org/linux/man-pages/man2/open.2.html>
#[derive(Debug)]
pub struct DirectIoFile {
    file: File,
    /// Scratch buffer of aligned memory for reads and writes
    scratch_buffer: Mutex<Vec<AlignedPage>>,
}

impl DirectIoFile {
    /// Open a file with basic open options at the specified path for direct/unbuffered I/O for
    /// reads and writes.
    ///
    /// `options` allows configuring things like read/write/create/truncate, but custom options
    /// will be overridden internally.
    ///
    /// This is especially important on Windows to prevent huge memory usage.
    #[inline]
    pub fn open<P>(
        #[cfg(any(target_os = "linux", windows))] mut options: OpenOptions,
        #[cfg(not(any(target_os = "linux", windows)))] options: OpenOptions,
        path: P,
    ) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        // Direct I/O on Linux
        #[cfg(target_os = "linux")]
        // TODO: Unlock under Miri once supported: https://github.com/rust-lang/miri/issues/4462
        if !cfg!(miri) {
            use std::os::unix::fs::OpenOptionsExt;

            options.custom_flags(libc::O_DIRECT);
        }
        // Unbuffered write-through on Windows
        #[cfg(windows)]
        // TODO: Unlock under Miri once supported: https://github.com/rust-lang/miri/issues/4462
        if !cfg!(miri) {
            use std::os::windows::fs::OpenOptionsExt;

            options.custom_flags(
                windows::Win32::Storage::FileSystem::FILE_FLAG_WRITE_THROUGH.0
                    | windows::Win32::Storage::FileSystem::FILE_FLAG_NO_BUFFERING.0,
            );
        }
        let file = options.open(path)?;

        // Disable caching on macOS
        #[cfg(target_os = "macos")]
        // TODO: Unlock under Miri once supported: https://github.com/rust-lang/miri/issues/4462
        if !cfg!(miri) {
            use std::os::unix::io::AsRawFd;

            // SAFETY: FFI call with correct file descriptor and arguments
            if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(Self {
            file,
            // In many cases, we'll want to read this much at once, so pre-allocate it right away
            scratch_buffer: Mutex::new(vec![
                AlignedPage::default();
                MAX_READ_SIZE / AlignedPage::SIZE
            ]),
        })
    }

    /// Get file size
    #[inline]
    pub fn len(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }

    /// Returns `Ok(true)` if the file is empty
    #[inline]
    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Make sure the file has a specified number of bytes allocated on the disk.
    ///
    /// Later writes within `len` will not fail due to lack of disk space.
    #[inline(always)]
    pub fn allocate(&self, len: u64) -> io::Result<()> {
        fs4::FileExt::allocate(&self.file, len)
    }

    /// Truncates or extends the underlying file, updating the size of this file to become `len`.
    ///
    /// Note if `len` is larger than the previous file size, it will result in a sparse file. If
    /// you'd like to pre-allocate space on disk, use [`Self::allocate()`], which may be followed by
    /// this method to truncate the file if the new file size is smaller than the previous
    /// ([`Self::allocate()`] doesn't truncate the file).
    #[inline(always)]
    pub fn set_len(&self, len: u64) -> io::Result<()> {
        self.file.set_len(len)
    }

    /// Read the exact number of bytes needed to fill `buf` at `offset`.
    ///
    /// NOTE: This uses locking and buffering internally, prefer [`Self::read_exact_at_raw()`] if
    /// you can control data alignment.
    pub fn read_exact_at(&self, buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut scratch_buffer = self.scratch_buffer.lock();

        // This is guaranteed by the constructor
        debug_assert!(
            AlignedPage::slice_to_repr(&scratch_buffer)
                .as_flattened()
                .len()
                <= MAX_READ_SIZE
        );

        // First read up to `MAX_READ_SIZE - padding`
        let padding = (offset % AlignedPage::SIZE as u64) as usize;
        let first_unaligned_chunk_size = (MAX_READ_SIZE - padding).min(buf.len());
        let (unaligned_start, buf) = buf.split_at_mut(first_unaligned_chunk_size);
        {
            let bytes_to_read = unaligned_start.len();
            unaligned_start.copy_from_slice(self.read_exact_at_internal(
                &mut scratch_buffer,
                bytes_to_read,
                offset,
            )?);
            offset += unaligned_start.len() as u64;
        }

        if buf.is_empty() {
            return Ok(());
        }

        // Process the rest of the chunks, up to `MAX_READ_SIZE` at a time
        for buf in buf.chunks_mut(MAX_READ_SIZE) {
            let bytes_to_read = buf.len();
            buf.copy_from_slice(self.read_exact_at_internal(
                &mut scratch_buffer,
                bytes_to_read,
                offset,
            )?);
            offset += buf.len() as u64;
        }

        Ok(())
    }

    /// Write all bytes at `buf` at `offset`.
    ///
    /// NOTE: This uses locking and buffering internally, prefer [`Self::write_all_at_raw()`] if you
    /// can control data alignment.
    pub fn write_all_at(&self, buf: &[u8], mut offset: u64) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut scratch_buffer = self.scratch_buffer.lock();

        // This is guaranteed by the constructor
        debug_assert!(
            AlignedPage::slice_to_repr(&scratch_buffer)
                .as_flattened()
                .len()
                <= MAX_READ_SIZE
        );

        // First, write up to `MAX_READ_SIZE - padding`
        let padding = (offset % AlignedPage::SIZE as u64) as usize;
        let first_unaligned_chunk_size = (MAX_READ_SIZE - padding).min(buf.len());
        let (unaligned_start, buf) = buf.split_at(first_unaligned_chunk_size);
        {
            self.write_all_at_internal(&mut scratch_buffer, unaligned_start, offset)?;
            offset += unaligned_start.len() as u64;
        }

        if buf.is_empty() {
            return Ok(());
        }

        // Process the rest of the chunks, up to `MAX_READ_SIZE` at a time
        for buf in buf.chunks(MAX_READ_SIZE) {
            self.write_all_at_internal(&mut scratch_buffer, buf, offset)?;
            offset += buf.len() as u64;
        }

        Ok(())
    }

    /// Low-level reading into aligned memory.
    ///
    /// `offset` needs to be page-aligned as well or use [`Self::read_exact_at()`] if you're willing
    /// to pay for the corresponding overhead.
    ///
    /// Successful result guarantees that the whole `buf` was filled.
    #[inline]
    pub fn read_exact_at_raw(
        &self,
        mut buf: BorrowedCursor<'_, AlignedPage>,
        offset: u64,
    ) -> io::Result<()> {
        // SAFETY: Only bytes read from the file are written into pages
        let pages = unsafe { buf.as_mut() };
        let num_pages = pages.len();
        let mut bytes = BorrowedBuf::from(pages.as_bytes_mut());

        cfg_select! {
            unix => {
                use std::os::unix::fs::FileExt;

                self.file.read_buf_exact_at(bytes.unfilled(), offset)?;
            }
            windows => {
                use std::os::windows::fs::FileExt;

                self.file.seek_read_buf_exact(bytes.unfilled(), offset)?;
            }
            _ => {
                compile_error!("Unsupported platform (consider contributing)");
            }
        }

        // SAFETY: All pages were just filled with bytes read from the file
        unsafe {
            buf.advance(num_pages);
        }

        Ok(())
    }

    /// Low-level writing from aligned memory.
    ///
    /// `offset` needs to be page-aligned as well or use [`Self::write_all_at()`] if you're willing
    /// to pay for the corresponding overhead.
    #[inline]
    pub fn write_all_at_raw(&self, buf: &[AlignedPage], offset: u64) -> io::Result<()> {
        let buf = AlignedPage::slice_to_repr(buf).as_flattened();

        cfg_select! {
            unix => {
                use std::os::unix::fs::FileExt;

                self.file.write_all_at(buf, offset)
            }
            windows => {
                use std::os::windows::fs::FileExt;

                self.file.seek_write_all(buf, offset)
            }
            _ => {
                compile_error!("Unsupported platform (consider contributing)");
            }
        }
    }

    /// Read `len` bytes at `offset` into `buffer`, replacing its contents.
    ///
    /// Whole pages that cover the requested bytes are read into the buffer, which grows if it
    /// doesn't have enough capacity. Existing contents are discarded, so they are never copied.
    /// The returned buffer contains exactly the requested bytes, which start at
    /// `offset % OwnedPages::ALIGN` from the beginning of the data region of its page-aligned
    /// allocation, see [`OwnedAlignedBuffer::into_empty()`] for reusing it for the next read.
    ///
    /// All pages that cover the requested bytes must exist in the file. The buffer is returned
    /// back on error too.
    #[inline]
    pub fn read_at(
        &self,
        mut buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        if len == 0 {
            buffer.truncate(0);
            return Ok(buffer.into_start_align());
        }

        let head = u32::try_from(offset % u64::from(OwnedPages::ALIGN))
            .expect("Smaller than the size of a page; qed");
        let Some((end, span)) = head
            .checked_add(len)
            .and_then(|end| Some((end, end.checked_next_multiple_of(OwnedPages::ALIGN)?)))
        else {
            return Err(IoBufError {
                error: AllocationError::CapacityOverflow.into(),
                buffer,
            });
        };

        if let Err(error) = self.read_pages_into(&mut buffer, span, offset - u64::from(head)) {
            return Err(IoBufError { error, buffer });
        }

        let mut buffer = buffer.into_start_align::<Align1>();
        buffer.truncate(end);
        if !buffer.set_offset(head) {
            unreachable!(
                "Contents are `head + len` bytes at the beginning of the data region; qed"
            );
        }
        Ok(buffer)
    }

    /// Read `num_pages` pages at `page_offset` (in pages) into `buffer`, replacing its contents.
    ///
    /// The buffer grows if it doesn't have enough capacity. Existing contents are discarded, so
    /// they are never copied. All pages must exist in the file. The buffer is returned back on
    /// error too.
    #[inline]
    pub fn read_pages_at(
        &self,
        mut buffer: OwnedPages,
        num_pages: u32,
        page_offset: u64,
    ) -> Result<OwnedPages, IoBufError<OwnedPages>> {
        let Some(len) = num_pages.checked_mul(OwnedPages::ALIGN) else {
            return Err(IoBufError {
                error: AllocationError::CapacityOverflow.into(),
                buffer,
            });
        };
        let Some(offset) = page_offset.checked_mul(u64::from(OwnedPages::ALIGN)) else {
            return Err(IoBufError {
                error: io::Error::from(io::ErrorKind::InvalidInput).into(),
                buffer,
            });
        };

        match self.read_pages_into(&mut buffer, len, offset) {
            Ok(()) => Ok(buffer),
            Err(error) => Err(IoBufError { error, buffer }),
        }
    }

    /// Write the contents of `buffer` at `offset`.
    ///
    /// The contents must be as unaligned in memory as `offset` is in the file, both must have the
    /// same offset within a page, which is the case for a buffer returned by [`Self::read_at()`]
    /// at the same offset and for page-aligned buffers written at page-aligned offsets, otherwise
    /// [`io::ErrorKind::InvalidInput`] is returned. Whole pages are written directly from the
    /// buffer. Partially written pages at the beginning and the end are read from the file first,
    /// so they must exist in the file, and are written from a page on the stack, serialized with
    /// other such writes. The buffer is returned back on error too.
    #[inline]
    pub fn write_at<B>(&self, buffer: B, offset: u64) -> Result<B, IoBufError<B>>
    where
        B: AlignedBuffer,
    {
        match self.write_bytes(&buffer, offset) {
            Ok(()) => Ok(buffer),
            Err(error) => Err(IoBufError {
                error: error.into(),
                buffer,
            }),
        }
    }

    /// Read `len` bytes (whole pages) at page-aligned `offset` into `buffer`, replacing its
    /// contents
    fn read_pages_into(
        &self,
        buffer: &mut OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<(), DirectIoError> {
        // Content is replaced and moved to the beginning of the data region, so nothing is copied
        // if the buffer has to grow
        *buffer = mem::take(buffer).into_empty();
        buffer.try_ensure_capacity(len)?;

        self.read_exact(
            BorrowedBuf::from(&mut buffer.spare_capacity_mut()[..len as usize]),
            offset,
        )?;
        // SAFETY: `len` bytes were just read
        unsafe {
            buffer.set_len(len);
        }

        Ok(())
    }

    /// Read exactly the capacity of `buf` at `offset`, `buf` and `offset` are page-aligned and the
    /// capacity of `buf` is a multiple of the page size
    fn read_exact(&self, mut buf: BorrowedBuf<'_, u8>, offset: u64) -> io::Result<()> {
        debug_assert!(
            offset.is_multiple_of(u64::from(OwnedPages::ALIGN))
                && buf.capacity().is_multiple_of(OwnedPages::ALIGN as usize)
                && buf
                    .filled()
                    .as_ptr()
                    .addr()
                    .is_multiple_of(OwnedPages::ALIGN as usize),
            "Direct I/O requires page alignment"
        );

        cfg_select! {
            unix => self.file.read_buf_exact_at(buf.unfilled(), offset),
            windows => self.file.seek_read_buf_exact(buf.unfilled(), offset),
            _ => {
                compile_error!("Unsupported platform (consider contributing)")
            }
        }
    }

    /// Write `bytes` at `offset`, both have the same offset within a page
    fn write_bytes(&self, mut bytes: &[u8], mut offset: u64) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }

        let page = OwnedPages::ALIGN as usize;
        let head = usize::try_from(offset % u64::from(OwnedPages::ALIGN))
            .expect("Smaller than the size of a page; qed");
        if bytes.as_ptr().addr() % page != head {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Bytes have a different offset within a page than the file offset",
            ));
        }

        // Offsets are only advanced after a successful write at a smaller offset, which the OS
        // only accepts far below `u64::MAX`, so they don't overflow
        if head != 0 {
            let prefix;
            (prefix, bytes) = bytes.split_at((page - head).min(bytes.len()));
            let page_offset = offset - offset % u64::from(OwnedPages::ALIGN);
            self.write_partial_page(prefix, page_offset, head)?;
            offset = page_offset + u64::from(OwnedPages::ALIGN);
        }

        let middle;
        (middle, bytes) = bytes.split_at(bytes.len() - bytes.len() % page);
        if !middle.is_empty() {
            self.write_pages(middle, offset)?;
            offset += u64::try_from(middle.len()).expect("Buffers are smaller than 4 GiB; qed");
        }

        if !bytes.is_empty() {
            self.write_partial_page(bytes, offset, 0)?;
        }

        Ok(())
    }

    /// Write whole pages at page-aligned `offset`, `bytes` are page-aligned in memory
    fn write_pages(&self, bytes: &[u8], offset: u64) -> io::Result<()> {
        debug_assert!(
            offset.is_multiple_of(u64::from(OwnedPages::ALIGN))
                && bytes.len().is_multiple_of(OwnedPages::ALIGN as usize)
                && bytes
                    .as_ptr()
                    .addr()
                    .is_multiple_of(OwnedPages::ALIGN as usize),
            "Direct I/O requires page alignment"
        );

        cfg_select! {
            unix => self.file.write_all_at(bytes, offset),
            windows => self.file.seek_write_all(bytes, offset),
            _ => {
                compile_error!("Unsupported platform (consider contributing)")
            }
        }
    }

    /// Write `bytes` at `start` within the page at page-aligned `page_offset`, the rest of the
    /// page is read from the file first
    fn write_partial_page(&self, bytes: &[u8], page_offset: u64, start: usize) -> io::Result<()> {
        let mut page = Page([0; _]);

        // Read-modify-write of pages is serialized, such that concurrent writes to different bytes
        // of the same page don't overwrite each other
        let _guard = self.scratch_buffer.lock();

        self.read_exact(BorrowedBuf::from(page.0.as_mut_slice()), page_offset)?;
        page.0[start..][..bytes.len()].copy_from_slice(bytes);
        self.write_pages(&page.0, page_offset)
    }

    /// Access internal [`File`] instance
    #[inline(always)]
    pub fn file(&self) -> &File {
        &self.file
    }

    fn read_exact_at_internal<'a>(
        &self,
        scratch_buffer: &'a mut [AlignedPage],
        bytes_to_read: usize,
        offset: u64,
    ) -> io::Result<&'a [u8]> {
        let page_aligned_offset = offset / AlignedPage::SIZE as u64 * AlignedPage::SIZE as u64;
        let padding = (offset - page_aligned_offset) as usize;

        // Make a scratch buffer of a size that is necessary to read aligned memory, accounting
        // for extra bytes at the beginning and the end that will be thrown away
        let pages_to_read = (padding + bytes_to_read).div_ceil(AlignedPage::SIZE);
        let scratch_buffer = &mut scratch_buffer[..pages_to_read];

        self.read_exact_at_raw(
            BorrowedBuf::from(&mut *scratch_buffer).unfilled(),
            page_aligned_offset,
        )?;

        Ok(&AlignedPage::slice_to_repr(scratch_buffer).as_flattened()[padding..][..bytes_to_read])
    }

    /// Panics on writes over `MAX_READ_SIZE` (including padding on both ends)
    fn write_all_at_internal(
        &self,
        scratch_buffer: &mut [AlignedPage],
        bytes_to_write: &[u8],
        offset: u64,
    ) -> io::Result<()> {
        let page_aligned_offset = offset / AlignedPage::SIZE as u64 * AlignedPage::SIZE as u64;
        let padding = (offset - page_aligned_offset) as usize;

        // Calculate the size of the read including padding on both ends
        let pages_to_read = (padding + bytes_to_write.len()).div_ceil(AlignedPage::SIZE);

        if padding == 0 && bytes_to_write.len() == pages_to_read * AlignedPage::SIZE {
            let scratch_buffer = &mut scratch_buffer[..pages_to_read];
            AlignedPage::slice_mut_to_repr(scratch_buffer)
                .as_flattened_mut()
                .copy_from_slice(bytes_to_write);
            self.write_all_at_raw(scratch_buffer, offset)?;
        } else {
            let scratch_buffer = &mut scratch_buffer[..pages_to_read];
            // Read whole pages where `bytes_to_write` will be written
            self.read_exact_at_raw(
                BorrowedBuf::from(&mut *scratch_buffer).unfilled(),
                page_aligned_offset,
            )?;
            // Update the contents of existing pages and write into the file
            AlignedPage::slice_mut_to_repr(scratch_buffer).as_flattened_mut()[padding..]
                [..bytes_to_write.len()]
                .copy_from_slice(bytes_to_write);
            self.write_all_at_raw(scratch_buffer, page_aligned_offset)?;
        }

        Ok(())
    }
}
