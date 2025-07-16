//! Wrapper data structure for direct/unbuffered I/O

use ab_direct_io_file::{AlignedPageSize, DirectIoFile};
use ab_farmer_components::ReadAtSync;
use ab_farmer_components::file_ext::FileExt;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;

/// 4096 is as a relatively safe size due to sector size on SSDs commonly being 512 or 4096 bytes
pub const DISK_PAGE_SIZE: usize = AlignedPageSize::SIZE;

/// Wrapper data structure for direct/unbuffered I/O
#[derive(Debug)]
pub struct DirectIoFileWrapper {
    file: DirectIoFile,
}

impl ReadAtSync for DirectIoFileWrapper {
    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.read_exact_at(buf, offset)
    }
}

impl ReadAtSync for &DirectIoFileWrapper {
    #[inline]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        (*self).read_at(buf, offset)
    }
}

impl FileExt for DirectIoFileWrapper {
    fn size(&self) -> io::Result<u64> {
        self.file.len()
    }

    fn preallocate(&self, len: u64) -> io::Result<()> {
        self.file.allocate(len)
    }

    fn advise_random_access(&self) -> io::Result<()> {
        // Ignore, already set
        Ok(())
    }

    fn advise_sequential_access(&self) -> io::Result<()> {
        // Ignore, not supported
        Ok(())
    }

    fn disable_cache(&self) -> io::Result<()> {
        // Ignore, not supported
        Ok(())
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        self.file.read_exact_at(buf, offset)
    }

    fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        self.file.write_all_at(buf, offset)
    }
}

impl DirectIoFileWrapper {
    /// Open file at specified path for direct/unbuffered I/O for reads (if file doesn't exist, it
    /// will be created).
    ///
    /// This is especially important on Windows to prevent huge memory usage.
    pub fn open<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut open_options = OpenOptions::new();
        open_options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false);

        let file = DirectIoFile::open(open_options, path)?;

        Ok(Self { file })
    }

    /// Truncates or extends the underlying file, updating the size of this file to become `size`.
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        self.file.set_len(size)
    }
}
