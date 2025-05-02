#![feature(
    box_vec_non_null,
    non_null_from_ref,
    pointer_is_aligned_to,
    ptr_as_ref_unchecked
)]
#![no_std]

#[cfg(test)]
mod tests;

extern crate alloc;

use ab_io_type::MAX_ALIGNMENT;
use alloc::boxed::Box;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::{AtomicU32, Ordering};

const _: () = {
    assert!(
        align_of::<u128>() == size_of::<u128>(),
        "Size and alignment are both 16 bytes"
    );
    assert!(
        align_of::<u128>() == MAX_ALIGNMENT as usize,
        "Alignment of u128 is max alignment"
    );
    assert!(size_of::<u128>() >= size_of::<AtomicU32>());
    assert!(align_of::<u128>() >= align_of::<AtomicU32>());
};

#[repr(C, align(16))]
struct ConstInnerBuffer {
    strong_count: AtomicU32,
}

const _: () = {
    assert!(align_of::<ConstInnerBuffer>() == align_of::<u128>());
    assert!(size_of::<ConstInnerBuffer>() == size_of::<u128>());
};

static EMPTY_SHARED_ALIGNED_BUFFER: SharedAlignedBuffer = SharedAlignedBuffer {
    inner: InnerBuffer {
        buffer: NonNull::from_ref({
            static BUFFER: MaybeUninit<ConstInnerBuffer> = MaybeUninit::new(ConstInnerBuffer {
                strong_count: AtomicU32::new(1),
            });

            &BUFFER
        })
        .cast::<MaybeUninit<u128>>(),
        capacity: 0,
        len: 0,
    },
};

#[derive(Debug)]
struct InnerBuffer {
    // The first bytes are allocated for `strong_count`
    buffer: NonNull<MaybeUninit<u128>>,
    capacity: u32,
    len: u32,
}

// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Send for InnerBuffer {}
// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Sync for InnerBuffer {}

impl Default for InnerBuffer {
    #[inline(always)]
    fn default() -> Self {
        EMPTY_SHARED_ALIGNED_BUFFER.inner.clone()
    }
}

impl Clone for InnerBuffer {
    #[inline(always)]
    fn clone(&self) -> Self {
        self.strong_count_ref().fetch_add(1, Ordering::AcqRel);

        Self {
            buffer: self.buffer,
            capacity: self.capacity,
            len: self.len,
        }
    }
}

impl Drop for InnerBuffer {
    #[inline(always)]
    fn drop(&mut self) {
        if self.strong_count_ref().fetch_sub(1, Ordering::AcqRel) == 1 {
            // SAFETY: Created from `Box` in constructor
            let _ = unsafe {
                Box::from_non_null(NonNull::slice_from_raw_parts(
                    self.buffer,
                    1 + (self.capacity as usize).div_ceil(size_of::<u128>()),
                ))
            };
        }
    }
}

impl InnerBuffer {
    /// Allocates a new buffer + one `u128` worth of memory at the beginning for
    /// `strong_count` in case it is later converted to [`SharedAlignedBuffer`].
    ///
    /// `strong_count` field is automatically initialized as `1`.
    #[inline(always)]
    fn allocate(capacity: u32) -> Self {
        let buffer = Box::into_non_null(Box::<[u128]>::new_uninit_slice(
            1 + (capacity as usize).div_ceil(size_of::<u128>()),
        ));
        // SAFETY: The first bytes are allocated for `strong_count`, which is a correctly aligned
        // copy type
        unsafe { buffer.cast::<AtomicU32>().write(AtomicU32::new(1)) };
        Self {
            buffer: buffer.cast::<MaybeUninit<u128>>(),
            capacity,
            len: 0,
        }
    }

    #[inline(always)]
    fn len(&self) -> u32 {
        self.len
    }

    /// `len` bytes must be initialized
    #[inline(always)]
    unsafe fn set_len(&mut self, len: u32) {
        self.len = len;
    }

    #[inline(always)]
    fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline(always)]
    fn strong_count_ref(&self) -> &AtomicU32 {
        // SAFETY: The first bytes are allocated for `strong_count`, which is a correctly aligned
        // copy type initialized in the constructor
        unsafe { self.buffer.as_ptr().cast::<AtomicU32>().as_ref_unchecked() }
    }

    #[inline(always)]
    fn as_slice(&self) -> &[u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), len) }
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const u8 {
        // SAFETY: Constructor allocates the first element for `strong_count`
        unsafe { self.buffer.as_ptr().cast_const().add(1).cast::<u8>() }
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        // SAFETY: Constructor allocates the first element for `strong_count`
        unsafe { self.buffer.as_ptr().add(1).cast::<u8>() }
    }
}

/// Owned aligned buffer for executor purposes.
///
/// See [`SharedAlignedBuffer`] for a version that can be cheaply cloned, while reusing the original
/// allocation.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
pub struct OwnedAlignedBuffer {
    inner: InnerBuffer,
}

impl Clone for OwnedAlignedBuffer {
    #[inline(always)]
    fn clone(&self) -> Self {
        let mut new_instance = Self::with_capacity(self.capacity());
        new_instance.copy_from_slice(self.as_slice());
        new_instance
    }
}

impl OwnedAlignedBuffer {
    /// Create a new instance with at least specified capacity.
    ///
    /// NOTE: Actual capacity might be larger due to alignment requirements.
    #[inline(always)]
    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            inner: InnerBuffer::allocate(capacity),
        }
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut instance = Self::with_capacity(0);
        instance.copy_from_slice(bytes);
        instance
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner.as_mut_slice()
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }

    #[inline(always)]
    pub fn into_shared(self) -> SharedAlignedBuffer {
        SharedAlignedBuffer { inner: self.inner }
    }

    /// Ensure capacity of the buffer is at least `capacity`.
    ///
    /// Will re-allocate if necessary.
    #[inline(always)]
    pub fn ensure_capacity(&mut self, capacity: u32) {
        // TODO: Try to `realloc()` if possible
        if capacity > self.capacity() {
            let mut new_buffer = Self::with_capacity(capacity);
            new_buffer.copy_from_slice(self.as_slice());
            *self = new_buffer;
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        // TODO: Try to `realloc()` if possible
        if len > self.capacity() {
            // Allocate new buffer
            self.inner = InnerBuffer::allocate(len);
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by type system
        unsafe {
            self.as_mut_ptr()
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());

            self.inner.set_len(len);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// Returns `false` if `self.len() + bytes.len()` doesn't fit into `u32`.
    #[inline(always)]
    #[must_use]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };

        let Some(new_len) = self.len().checked_add(len) else {
            return false;
        };

        // TODO: Try to `realloc()` if possible
        if new_len > self.capacity() {
            // Allocate new buffer
            self.inner = InnerBuffer::allocate(new_len.max(self.capacity() * 2));
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by type system
        unsafe {
            self.as_mut_ptr()
                .add(self.len() as usize)
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());

            self.inner.set_len(new_len);
        }

        true
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    #[inline(always)]
    pub fn len(&self) -> u32 {
        self.inner.len()
    }

    #[inline(always)]
    pub fn capacity(&self) -> u32 {
        self.inner.capacity()
    }

    /// Set the length of the useful data to specified value.
    ///
    /// # Safety
    /// There must be `new_len` bytes initialized in the buffer.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub unsafe fn set_len(&mut self, new_len: u32) {
        assert!(
            new_len <= self.capacity(),
            "Too many bytes {} > {}",
            new_len,
            self.capacity()
        );
        // SAFETY: Guaranteed by method contract
        unsafe {
            self.inner.set_len(new_len);
        }
    }
}

/// Shared aligned buffer for executor purposes.
///
/// See [`OwnedAlignedBuffer`] for a version that can be mutated.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
///
/// NOTE: Counter for number of shared instances is `u32` and will wrap around if exceeded breaking
/// internal invariants (which is extremely unlikely, but still).
#[derive(Debug, Default, Clone)]
pub struct SharedAlignedBuffer {
    inner: InnerBuffer,
}

// SAFETY: Heap-allocated memory buffer and atomic can be used from any thread
unsafe impl Send for SharedAlignedBuffer {}
// SAFETY: Heap-allocated memory buffer and atomic can be used from any thread
unsafe impl Sync for SharedAlignedBuffer {}

impl SharedAlignedBuffer {
    /// Static reference to an empty buffer
    #[inline(always)]
    pub fn empty_ref() -> &'static Self {
        &EMPTY_SHARED_ALIGNED_BUFFER
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        OwnedAlignedBuffer::from_bytes(bytes).into_shared()
    }

    /// Convert into owned buffer.
    ///
    /// If this is the last shared instance, then allocation will be reused, otherwise new
    /// allocation will be created.
    ///
    /// Returns `None` if there exit other shared instances.
    #[inline(always)]
    pub fn into_owned(self) -> OwnedAlignedBuffer {
        if self.inner.strong_count_ref().load(Ordering::Acquire) == 1 {
            OwnedAlignedBuffer { inner: self.inner }
        } else {
            OwnedAlignedBuffer::from_bytes(self.as_slice())
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    #[inline(always)]
    pub fn len(&self) -> u32 {
        self.inner.len()
    }
}
