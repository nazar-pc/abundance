#[cfg(test)]
mod tests;

use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::{mem, slice};

#[repr(C, align(16))]
struct AlignedBytes([u8; Self::SIZE]);

impl AlignedBytes {
    const SIZE: usize = 16;
}

/// Owned aligned buffer for executor purposes.
///
/// See [`SharedAlignedBuffer`] for a version that can be cheaply cloned, while reusing the original
/// allocation.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
pub(super) struct OwnedAlignedBuffer {
    buffer: Arc<[MaybeUninit<AlignedBytes>]>,
    len: u32,
}

impl Deref for OwnedAlignedBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for OwnedAlignedBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl PartialEq for OwnedAlignedBuffer {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl Eq for OwnedAlignedBuffer {}

impl OwnedAlignedBuffer {
    /// Create a new instance with at least specified capacity.
    ///
    /// NOTE: Actual capacity might be larger due to alignment requirements.
    #[inline]
    pub(super) fn with_capacity(capacity: u32) -> Self {
        Self {
            buffer: Arc::new_uninit_slice((capacity as usize).div_ceil(AlignedBytes::SIZE)),
            len: 0,
        }
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline]
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        let mut instance = Self::with_capacity(0);
        instance.copy_from_slice(bytes);
        instance
    }

    #[inline]
    pub(super) fn as_slice(&self) -> &[u8] {
        let len = self.len as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline]
    pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), len) }
    }

    #[inline]
    pub(super) fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr().cast::<u8>()
    }

    #[inline]
    pub(super) fn as_mut_ptr(&mut self) -> *mut u8 {
        Arc::get_mut(&mut self.buffer)
            .expect("Owned by this data structure; qed")
            .as_mut_ptr()
            .cast::<u8>()
    }

    #[inline]
    pub(super) fn into_shared(self) -> SharedAlignedBuffer {
        SharedAlignedBuffer {
            buffer: self.buffer,
            len: self.len,
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline]
    pub(super) fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes");
        };

        if len > self.capacity() {
            // Drop old buffer
            mem::take(&mut self.buffer);
            // Allocate new buffer
            self.buffer = Self::with_capacity(len).buffer;
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by type system
        unsafe {
            self.as_mut_ptr()
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
        }

        self.len = len;
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub(super) fn capacity(&self) -> u32 {
        (self.buffer.len() * AlignedBytes::SIZE) as u32
    }

    /// Set the length of the useful data to specified value.
    ///
    /// # Safety
    /// There must be `new_len` bytes initialized in the buffer.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline]
    pub(super) unsafe fn set_len(&mut self, new_len: u32) {
        if new_len > self.capacity() {
            panic!("Too many bytes");
        }
        self.len = new_len;
    }
}

/// Shared aligned buffer for executor purposes.
///
/// See [`OwnedAlignedBuffer`] for a version that can be mutated.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug, Default, Clone)]
pub(super) struct SharedAlignedBuffer {
    buffer: Arc<[MaybeUninit<AlignedBytes>]>,
    len: u32,
}

impl Deref for SharedAlignedBuffer {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl PartialEq for SharedAlignedBuffer {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl PartialEq<OwnedAlignedBuffer> for SharedAlignedBuffer {
    #[inline]
    fn eq(&self, other: &OwnedAlignedBuffer) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl PartialEq<SharedAlignedBuffer> for OwnedAlignedBuffer {
    #[inline]
    fn eq(&self, other: &SharedAlignedBuffer) -> bool {
        self.len == other.len && self.as_slice() == other.as_slice()
    }
}

impl Eq for SharedAlignedBuffer {}

impl SharedAlignedBuffer {
    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline]
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        OwnedAlignedBuffer::from_bytes(bytes).into_shared()
    }

    /// Convert into owned buffer.
    ///
    /// If this is the last shared instance, then allocation will be reused, otherwise new
    /// allocation will be created.
    ///
    /// Returns `None` if there exit other shared instances.
    #[inline]
    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub(super) fn into_owned(mut self) -> OwnedAlignedBuffer {
        // Check if this is the last instance of the buffer
        if Arc::get_mut(&mut self.buffer).is_some() {
            OwnedAlignedBuffer {
                buffer: self.buffer,
                len: self.len,
            }
        } else {
            let mut instance = OwnedAlignedBuffer::with_capacity(self.capacity());
            instance.copy_from_slice(self.as_slice());
            instance
        }
    }

    #[inline]
    pub(super) fn as_slice(&self) -> &[u8] {
        // SAFETY: Not null and size is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.buffer.as_ptr().cast::<u8>(), self.len as usize) }
    }

    #[inline]
    pub(super) fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr().cast::<u8>()
    }

    #[inline]
    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub(super) fn capacity(&self) -> u32 {
        (self.buffer.len() * AlignedBytes::SIZE) as u32
    }
}
