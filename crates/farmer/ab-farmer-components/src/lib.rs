//! Components of the reference implementation of Subspace Farmer for Subspace Network Blockchain.
//!
//! These components are used to implement farmer itself, but can also be used independently if
//! necessary.

#![feature(
    const_block_items,
    const_convert,
    const_trait_impl,
    iter_array_chunks,
    maybe_uninit_array_assume_init,
    maybe_uninit_fill,
    portable_simd,
    try_blocks
)]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]

pub mod auditing;
pub mod file_ext;
pub mod plotting;
pub mod proving;
pub mod reading;
pub mod sector;
mod segment_reconstruction;
pub mod shard_commitment;

use crate::file_ext::FileExt;
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_aligned_buffer::alignment::Align1;
use ab_core_primitives::segments::HistorySize;
use ab_direct_io_file::{DirectIoFile, IoBufError, OwnedPages};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::future::Future;
use std::io;

/// Enum to encapsulate the selection between [`ReadAtSync`] and [`ReadAtAsync]` variants
#[derive(Debug, Copy, Clone)]
pub enum ReadAt<S, A>
where
    S: ReadAtSync,
    A: ReadAtAsync,
{
    /// Sync variant
    Sync(S),
    /// Async variant
    Async(A),
}

impl<S> ReadAt<S, !>
where
    S: ReadAtSync,
{
    /// Instantiate [`ReadAt`] from some [`ReadAtSync`] implementation
    pub fn from_sync(value: S) -> Self {
        Self::Sync(value)
    }
}

impl<A> ReadAt<!, A>
where
    A: ReadAtAsync,
{
    /// Instantiate [`ReadAt`] from some [`ReadAtAsync`] implementation
    pub fn from_async(value: A) -> Self {
        Self::Async(value)
    }
}

/// Sync version of [`ReadAt`], it is both [`Send`] and [`Sync`] and is supposed to be used with a
/// thread pool
pub trait ReadAtSync: Send + Sync {
    /// Get implementation of [`ReadAtSync`] that add specified offset to all attempted reads
    fn offset(&self, offset: u64) -> ReadAtOffset<'_, Self>
    where
        Self: Sized,
    {
        ReadAtOffset {
            inner: self,
            offset,
        }
    }

    /// Read `len` bytes at `offset` into `buffer`, replacing its contents.
    ///
    /// The allocation of the buffer is reused when possible and grows otherwise, allocation
    /// failure is returned as an error. Existing contents are never copied. On success, the
    /// contents are exactly the requested bytes. The buffer is returned back on error too.
    ///
    /// The returned buffer can be reused for the next read with
    /// [`OwnedAlignedBuffer::into_empty()`].
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>>;
}

impl<T> ReadAtSync for &T
where
    T: ReadAtSync + ?Sized,
{
    #[inline]
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        (**self).read_at(buffer, len, offset)
    }
}

impl ReadAtSync for ! {
    fn read_at(
        &self,
        _buffer: OwnedPages,
        _len: u32,
        _offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        unreachable!("Is never called")
    }
}

impl ReadAtSync for [u8] {
    fn read_at(
        &self,
        mut buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        let maybe_bytes = offset
            .checked_add(u64::from(len))
            .and_then(|end| self.get(usize::try_from(offset).ok()?..usize::try_from(end).ok()?));
        let Some(bytes) = maybe_bytes else {
            return Err(IoBufError {
                error: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Buffer length with offset exceeds own length",
                )
                .into(),
                buffer,
            });
        };

        // Grow the buffer while it is page-aligned, such that the allocation can be reused for
        // further reads
        buffer.truncate(0);
        if let Err(error) = buffer.try_ensure_capacity(len) {
            return Err(IoBufError {
                error: error.into(),
                buffer,
            });
        }

        let mut contents = buffer.into_start_align::<Align1>();
        contents
            .append(bytes)
            .expect("Capacity for `len` bytes is ensured above; qed");

        Ok(contents)
    }
}

impl ReadAtSync for Vec<u8> {
    #[inline]
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        self.as_slice().read_at(buffer, len, offset)
    }
}

impl ReadAtSync for File {
    fn read_at(
        &self,
        mut buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        buffer.truncate(0);
        if let Err(error) = buffer.try_ensure_capacity(len) {
            return Err(IoBufError {
                error: error.into(),
                buffer,
            });
        }
        buffer.spare_capacity_mut()[..len as usize].write_filled(0);
        // SAFETY: Capacity for `len` bytes is ensured and they are initialized with zeroes above
        unsafe {
            buffer.set_len(len);
        }

        if let Err(error) = self.read_exact_at(buffer.as_mut_slice(), offset) {
            buffer.truncate(0);
            return Err(IoBufError {
                error: error.into(),
                buffer,
            });
        }

        Ok(buffer.into_start_align())
    }
}

impl ReadAtSync for DirectIoFile {
    #[inline]
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        DirectIoFile::read_at(self, buffer, len, offset)
    }
}

/// Async version of [`ReadAt`], it is neither [`Send`] nor [`Sync`] and is supposed to be used with
/// concurrent async combinators
pub trait ReadAtAsync {
    /// Get implementation of [`ReadAtAsync`] that add specified offset to all attempted reads
    fn offset(&self, offset: u64) -> ReadAtOffset<'_, Self>
    where
        Self: Sized,
    {
        ReadAtOffset {
            inner: self,
            offset,
        }
    }

    /// Read `len` bytes at `offset` into `buffer`, replacing its contents, see
    /// [`ReadAtSync::read_at()`] for details
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> impl Future<Output = Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>>>;
}

impl<T> ReadAtAsync for &T
where
    T: ReadAtAsync + ?Sized,
{
    #[inline]
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> impl Future<Output = Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>>> {
        (**self).read_at(buffer, len, offset)
    }
}

impl ReadAtAsync for ! {
    async fn read_at(
        &self,
        _buffer: OwnedPages,
        _len: u32,
        _offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        unreachable!("Is never called")
    }
}

/// Reader with fixed offset added to all attempted reads
#[derive(Debug, Copy, Clone)]
pub struct ReadAtOffset<'a, T> {
    inner: &'a T,
    offset: u64,
}

impl<T> ReadAtSync for ReadAtOffset<'_, T>
where
    T: ReadAtSync,
{
    #[inline]
    fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        let Some(offset) = offset.checked_add(self.offset) else {
            return Err(IoBufError {
                error: io::Error::new(io::ErrorKind::InvalidInput, "Offset overflow").into(),
                buffer,
            });
        };

        self.inner.read_at(buffer, len, offset)
    }
}

impl<T> ReadAtAsync for ReadAtOffset<'_, T>
where
    T: ReadAtAsync,
{
    #[inline]
    async fn read_at(
        &self,
        buffer: OwnedPages,
        len: u32,
        offset: u64,
    ) -> Result<OwnedAlignedBuffer<Align1>, IoBufError<OwnedPages>> {
        let Some(offset) = offset.checked_add(self.offset) else {
            return Err(IoBufError {
                error: io::Error::new(io::ErrorKind::InvalidInput, "Offset overflow").into(),
                buffer,
            });
        };

        self.inner.read_at(buffer, len, offset).await
    }
}

// Refuse to compile on non-64-bit platforms, offsets may fail on those when converting from u64 to
// usize depending on chain parameters
const {
    assert!(size_of::<usize>() >= size_of::<u64>());
}

/// Information about the protocol necessary for farmer operation
#[derive(Debug, Copy, Clone, Encode, Decode, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FarmerProtocolInfo {
    /// Size of the blockchain history
    pub history_size: HistorySize,
    /// How many pieces one sector is supposed to contain (max)
    pub max_pieces_in_sector: u16,
    /// Number of latest archived segments that are considered "recent history".
    pub recent_segments: HistorySize,
    /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector.
    pub recent_history_fraction: (HistorySize, HistorySize),
    /// Minimum lifetime of a plotted sector, measured in archived segment
    pub min_sector_lifetime: HistorySize,
}
