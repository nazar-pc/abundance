//! Cross-platform APIs for working with files using direct I/O.
//!
//! Depending on OS, this will use direct I/O, unbuffered, uncaches passthrough file reads/writes,
//! bypassing as much of OS machinery as possible.
//!
//! NOTE: There are major alignment requirements described here:
//! <https://learn.microsoft.com/en-us/windows/win32/fileio/file-buffering#alignment-and-file-access-requirements>
//! <https://man7.org/linux/man-pages/man2/open.2.html>

// TODO: Windows shims are incomplete under Miri: https://github.com/rust-lang/miri/issues/3482
#[cfg(all(test, not(all(miri, windows))))]
mod tests;

use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::{io, mem};

/// 4096 is as a relatively safe size due to sector size on SSDs commonly being 512 or 4096 bytes
pub const DISK_PAGE_SIZE: usize = 4096;
/// Restrict how much data to read from disk in a single call to avoid very large memory usage
const MAX_READ_SIZE: usize = 1024 * 1024;

const _: () = {
    assert!(MAX_READ_SIZE.is_multiple_of(DISK_PAGE_SIZE));
};

/// A wrapper data structure with 4096 bytes alignment, which is the most common alignment for
/// direct I/O operations.
#[derive(Debug, Copy, Clone)]
#[repr(C, align(4096))]
pub struct AlignedPageSize([u8; DISK_PAGE_SIZE]);

const _: () = {
    assert!(align_of::<AlignedPageSize>() == DISK_PAGE_SIZE);
};

impl Default for AlignedPageSize {
    #[inline(always)]
    fn default() -> Self {
        Self([0; DISK_PAGE_SIZE])
    }
}

impl AlignedPageSize {
    /// Convenient conversion from slice to underlying representation for efficiency purposes
    #[inline(always)]
    pub fn slice_to_repr(value: &[Self]) -> &[[u8; DISK_PAGE_SIZE]] {
        // SAFETY: `RecordChunk` is `#[repr(C)]` and guaranteed to have the same memory layout
        unsafe { mem::transmute(value) }
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_from_repr(value: &[[u8; DISK_PAGE_SIZE]]) -> Option<&[Self]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to::<Self>() };

        if before.is_empty() && after.is_empty() {
            Some(slice)
        } else {
            None
        }
    }

    /// Convenient conversion from mutable slice to underlying representation for efficiency
    /// purposes
    #[inline(always)]
    pub fn slice_mut_to_repr(slice: &mut [Self]) -> &mut [[u8; DISK_PAGE_SIZE]] {
        // SAFETY: `AlignedSectorSize` is `#[repr(C)]` and its alignment is larger than inner value
        unsafe { mem::transmute(slice) }
    }

    /// Convenient conversion from slice of underlying representation for efficiency purposes.
    ///
    /// Returns `None` if not correctly aligned.
    #[inline]
    pub fn try_slice_mut_from_repr(value: &mut [[u8; DISK_PAGE_SIZE]]) -> Option<&mut [Self]> {
        // SAFETY: All bit patterns are valid
        let (before, slice, after) = unsafe { value.align_to_mut::<Self>() };

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
    scratch_buffer: Mutex<Vec<AlignedPageSize>>,
}

impl DirectIoFile {
    /// Open file with basic open options at specified path for direct/unbuffered I/O for reads and
    /// writes.
    ///
    /// `options` allows to configure things like read/write/create/truncate, but custom options
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
                AlignedPageSize::default();
                MAX_READ_SIZE / DISK_PAGE_SIZE
            ]),
        })
    }

    /// Get file size
    #[inline]
    pub fn len(&self) -> io::Result<u64> {
        Ok(self.file.metadata()?.len())
    }

    /// Returns `Ok(true)` if file is empty
    #[inline]
    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Make sure file has specified number of bytes allocated on the disk.
    ///
    /// Later writes within `len` will not fail due to lack of disk space.
    #[inline(always)]
    pub fn allocate(&self, len: u64) -> io::Result<()> {
        fs2::FileExt::allocate(&self.file, len)
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
    /// NOTE: This uses locking and buffering internally, prefer [`Self::write_all_at_raw()`] if you
    /// can control data alignment.
    pub fn read_exact_at(&self, buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut scratch_buffer = self.scratch_buffer.lock();

        // This is guaranteed by constructor
        debug_assert!(
            AlignedPageSize::slice_to_repr(&scratch_buffer)
                .as_flattened()
                .len()
                <= MAX_READ_SIZE
        );

        // First read up to `MAX_READ_SIZE - padding`
        let padding = (offset % DISK_PAGE_SIZE as u64) as usize;
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

        // This is guaranteed by constructor
        debug_assert!(
            AlignedPageSize::slice_to_repr(&scratch_buffer)
                .as_flattened()
                .len()
                <= MAX_READ_SIZE
        );

        // First write up to `MAX_READ_SIZE - padding`
        let padding = (offset % DISK_PAGE_SIZE as u64) as usize;
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
    #[inline]
    pub fn read_exact_at_raw(&self, buf: &mut [AlignedPageSize], offset: u64) -> io::Result<()> {
        let buf = AlignedPageSize::slice_mut_to_repr(buf).as_flattened_mut();

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;

            self.file.read_exact_at(buf, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;

            let mut buf = buf;
            let mut offset = offset;
            while !buf.is_empty() {
                match self.file.seek_read(buf, offset) {
                    Ok(0) => {
                        break;
                    }
                    Ok(n) => {
                        buf = &mut buf[n..];
                        offset += n as u64;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                        // Try again
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }

            if !buf.is_empty() {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to fill the whole buffer",
                ))
            } else {
                Ok(())
            }
        }
    }

    /// Low-level writing from aligned memory.
    ///
    /// `offset` needs to be page-aligned as well or use [`Self::write_all_at()`] if you're willing
    /// to pay for the corresponding overhead.
    #[inline]
    pub fn write_all_at_raw(&self, buf: &[AlignedPageSize], offset: u64) -> io::Result<()> {
        let buf = AlignedPageSize::slice_to_repr(buf).as_flattened();

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;

            self.file.write_all_at(buf, offset)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;

            let mut buf = buf;
            let mut offset = offset;
            while !buf.is_empty() {
                match self.file.seek_write(buf, offset) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write whole buffer",
                        ));
                    }
                    Ok(n) => {
                        buf = &buf[n..];
                        offset += n as u64;
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                        // Try again
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }

            Ok(())
        }
    }

    /// Access internal [`File`] instance
    #[inline(always)]
    pub fn file(&self) -> &File {
        &self.file
    }

    fn read_exact_at_internal<'a>(
        &self,
        scratch_buffer: &'a mut [AlignedPageSize],
        bytes_to_read: usize,
        offset: u64,
    ) -> io::Result<&'a [u8]> {
        let page_aligned_offset = offset / DISK_PAGE_SIZE as u64 * DISK_PAGE_SIZE as u64;
        let padding = (offset - page_aligned_offset) as usize;

        // Make scratch buffer of a size that is necessary to read aligned memory, accounting
        // for extra bytes at the beginning and the end that will be thrown away
        let pages_to_read = (padding + bytes_to_read).div_ceil(DISK_PAGE_SIZE);
        let scratch_buffer = &mut scratch_buffer[..pages_to_read];

        self.read_exact_at_raw(scratch_buffer, page_aligned_offset)?;

        Ok(
            &AlignedPageSize::slice_to_repr(scratch_buffer).as_flattened()[padding..]
                [..bytes_to_read],
        )
    }

    /// Panics on writes over `MAX_READ_SIZE` (including padding on both ends)
    fn write_all_at_internal(
        &self,
        scratch_buffer: &mut [AlignedPageSize],
        bytes_to_write: &[u8],
        offset: u64,
    ) -> io::Result<()> {
        let page_aligned_offset = offset / DISK_PAGE_SIZE as u64 * DISK_PAGE_SIZE as u64;
        let padding = (offset - page_aligned_offset) as usize;

        // Calculate the size of the read including padding on both ends
        let pages_to_read = (padding + bytes_to_write.len()).div_ceil(DISK_PAGE_SIZE);

        if padding == 0 && pages_to_read == bytes_to_write.len() {
            let scratch_buffer = &mut scratch_buffer[..pages_to_read];
            AlignedPageSize::slice_mut_to_repr(scratch_buffer)
                .as_flattened_mut()
                .copy_from_slice(bytes_to_write);
            self.write_all_at_raw(scratch_buffer, offset)?;
        } else {
            let scratch_buffer = &mut scratch_buffer[..pages_to_read];
            // Read whole pages where `bytes_to_write` will be written
            self.read_exact_at_raw(scratch_buffer, page_aligned_offset)?;
            // Update the contents of existing pages and write into the file
            AlignedPageSize::slice_mut_to_repr(scratch_buffer).as_flattened_mut()[padding..]
                [..bytes_to_write.len()]
                .copy_from_slice(bytes_to_write);
            self.write_all_at_raw(scratch_buffer, page_aligned_offset)?;
        }

        Ok(())
    }
}
