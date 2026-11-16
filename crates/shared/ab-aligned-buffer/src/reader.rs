//! Reading the contents of shared buffers front to back

#[cfg(test)]
mod tests;

use crate::SharedAlignedBuffer;
use crate::alignment::Alignment;
use core::fmt;
use core::num::NonZeroU32;

/// Error of a read with [`SharedAlignedBufferReader`]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReadError {
    /// The contents end before the read does
    NotEnoughBytes {
        /// How many more bytes the read needs.
        ///
        /// Counted from the position of the reader, so it is not larger than the number of bytes
        /// missing for the rest of the data, whose size the reader doesn't know.
        missing: NonZeroU32,
    },
    /// Bytes at the position of the reader are not aligned in memory as the read requires
    Misaligned {
        /// Required alignment in bytes
        alignment: u32,
    },
}

impl fmt::Display for ReadError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughBytes { missing } => write!(f, "Need {missing} more bytes"),
            Self::Misaligned { alignment } => {
                write!(f, "Bytes are not aligned to {alignment} bytes")
            }
        }
    }
}

impl core::error::Error for ReadError {}

/// Reads the contents of a [`SharedAlignedBuffer`] front to back, see
/// [`SharedAlignedBuffer::reader()`].
///
/// Bytes are borrowed from the buffer for as long as the buffer is borrowed, so any number of them
/// can be held at once, while [`Self::read_buffer()`] creates buffers that reference a part of the
/// contents without copying them.
///
/// A read that fails leaves the reader unchanged.
#[derive(Debug, Clone)]
pub struct SharedAlignedBufferReader<'a, A>
where
    A: Alignment,
{
    buffer: &'a SharedAlignedBuffer<A>,
    /// Never larger than the length of the buffer
    position: u32,
}

impl<'a, A> SharedAlignedBufferReader<'a, A>
where
    A: Alignment,
{
    #[inline(always)]
    pub(crate) const fn new(buffer: &'a SharedAlignedBuffer<A>) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }

    /// Number of bytes read so far, including padding
    #[inline(always)]
    pub const fn position(&self) -> u32 {
        self.position
    }

    /// Bytes that were not read yet
    #[inline(always)]
    pub fn remaining(&self) -> &'a [u8] {
        let contents: &'a [u8] = self.buffer.as_slice();
        contents
            .get(self.position as usize..)
            .expect("Position is not larger than the length of the buffer; qed")
    }

    /// Read padding up to the next multiple of `B` bytes from the beginning of the contents.
    ///
    /// Returns padding bytes, which are empty if the position is already a multiple of `B`, so
    /// that formats that require zero padding can check them.
    #[inline]
    pub fn read_to_alignment<B>(&mut self) -> Result<&'a [u8], ReadError>
    where
        B: Alignment,
    {
        let misalignment = self.position % B::ALIGN;
        let padding = if misalignment == 0 {
            0
        } else {
            B::ALIGN - misalignment
        };
        self.read_bytes(padding)
    }

    /// Read the next `len` bytes
    #[inline]
    pub fn read_bytes(&mut self, len: u32) -> Result<&'a [u8], ReadError> {
        let start = self.advance(len)?;
        let contents: &'a [u8] = self.buffer.as_slice();
        Ok(contents
            .get(start as usize..)
            .and_then(|bytes| bytes.get(..len as usize))
            .expect("Within the contents as checked above; qed"))
    }

    /// Read the next `N` bytes
    #[inline]
    pub fn read_bytes_array<const N: usize>(&mut self) -> Result<&'a [u8; N], ReadError> {
        const {
            assert!(
                N <= u32::MAX as usize,
                "Buffers are never longer than `u32::MAX` bytes"
            );
        }
        let bytes = self.read_bytes(u32::try_from(N).expect("Checked above; qed"))?;
        Ok(bytes.try_into().expect("Exactly `N` bytes; qed"))
    }

    /// Create a buffer that references the next `len` bytes without copying them.
    ///
    /// The bytes must be aligned to `B` in memory, [`Self::read_to_alignment()`] reads padding in
    /// front of them if necessary. The new buffer keeps the whole allocation alive unless it is
    /// empty, see [`SharedAlignedBuffer::into_standalone()`].
    #[inline]
    pub fn read_buffer<B>(&mut self, len: u32) -> Result<SharedAlignedBuffer<B>, ReadError>
    where
        B: Alignment,
    {
        self.check_alignment(B::ALIGN)?;
        let start = self.advance(len)?;
        if len == 0 {
            return Ok(SharedAlignedBuffer::EMPTY);
        }

        Ok(SharedAlignedBuffer::from_inner(
            self.buffer.inner.slice(start, len),
        ))
    }

    /// Check that bytes at the position of the reader are aligned to `alignment` in memory
    #[inline(always)]
    fn check_alignment(&self, alignment: u32) -> Result<(), ReadError> {
        if self
            .remaining()
            .as_ptr()
            .addr()
            .is_multiple_of(alignment as usize)
        {
            Ok(())
        } else {
            Err(ReadError::Misaligned { alignment })
        }
    }

    /// Advance by `len` bytes, returns the position before advancing
    #[inline(always)]
    fn advance(&mut self, len: u32) -> Result<u32, ReadError> {
        let remaining = self.buffer.len() - self.position;
        if len > remaining {
            return Err(ReadError::NotEnoughBytes {
                missing: NonZeroU32::new(len - remaining)
                    .expect("More bytes are needed than remain as checked above; qed"),
            });
        }

        let start = self.position;
        // Within the contents, whose length is a `u32`, as checked above
        self.position += len;
        Ok(start)
    }
}
