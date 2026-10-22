//! Efficient abstraction for memory buffers aligned to 16 bytes with both owned and shared
//! variants.
//!
//! Buffers are aligned to 16 bytes on every target.
//!
//! [`OwnedAlignedBuffer`] represents a memory location aligned to 16 bytes that can be modified.
//!
//! [`SharedAlignedBuffer`] can't be modified but supports cheap reference-counting clones (like
//! `Arc`, but much more efficient).
//!
//! Does not require a standard library (`no_std`) but does require allocator and atomics.

#![cfg_attr(test, feature(pointer_is_aligned_to))]
#![no_std]

#[cfg(test)]
mod tests;

extern crate alloc;

use alloc::alloc::realloc;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering};
use stable_deref_trait::{CloneStableDeref, StableDeref};
use yoke::CloneableCart;

/// Unit of allocation: 16 bytes aligned to 16 bytes
#[repr(C, align(16))]
struct Chunk([u8; 16]);

const _: () = {
    assert!(size_of::<Chunk>() == 16);
    assert!(align_of::<Chunk>() == 16);
    // Buffers are accessed as `u128`s
    assert!(align_of::<Chunk>() >= align_of::<u128>());
    // The first chunk of every allocation holds the reference counter
    assert!(size_of::<Chunk>() >= size_of::<AtomicUsize>());
    assert!(align_of::<Chunk>() >= align_of::<AtomicUsize>());
};

#[repr(C, align(16))]
struct ConstInnerBuffer {
    strong_count: AtomicUsize,
}

const _: () = {
    assert!(align_of::<ConstInnerBuffer>() == align_of::<Chunk>());
    assert!(size_of::<ConstInnerBuffer>() == size_of::<Chunk>());
};

static EMPTY_SHARED_ALIGNED_BUFFER: SharedAlignedBuffer = SharedAlignedBuffer {
    inner: InnerBuffer {
        buffer: NonNull::from_ref({
            static BUFFER: MaybeUninit<ConstInnerBuffer> = MaybeUninit::new(ConstInnerBuffer {
                strong_count: AtomicUsize::new(1),
            });

            &BUFFER
        })
        .cast::<MaybeUninit<Chunk>>(),
        capacity: 0,
        len: 0,
    },
};

#[derive(Debug)]
struct InnerBuffer {
    // The first chunk is allocated for `strong_count`
    buffer: NonNull<MaybeUninit<Chunk>>,
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
        let old_count = self.strong_count_ref().fetch_add(1, Ordering::AcqRel);
        // Leaked clones could wrap a counter narrower than 64 bits around, and a drop would then
        // free the buffer while other clones still use it
        if usize::BITS < u64::BITS && old_count > isize::MAX.cast_unsigned() {
            self.strong_count_ref().fetch_sub(1, Ordering::AcqRel);
            panic!("Too many clones of a shared aligned buffer");
        }

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
            let _: Box<_> = unsafe {
                Box::from_non_null(NonNull::slice_from_raw_parts(
                    self.buffer,
                    1 + (self.capacity as usize).div_ceil(size_of::<Chunk>()),
                ))
            };
        }
    }
}

impl InnerBuffer {
    /// Allocates a new buffer + one chunk of memory at the beginning for `strong_count` in case it
    /// is later converted to [`SharedAlignedBuffer`].
    ///
    /// `strong_count` field is automatically initialized as `1`.
    #[inline(always)]
    fn allocate(capacity: u32) -> Self {
        let buffer = Box::into_non_null(Box::<[Chunk]>::new_uninit_slice(
            1 + (capacity as usize).div_ceil(size_of::<Chunk>()),
        ));
        // SAFETY: The first chunk is allocated for `strong_count`, which is a correctly aligned
        // copy type
        unsafe {
            buffer.cast::<AtomicUsize>().write(AtomicUsize::new(1));
        }
        Self {
            buffer: buffer.cast::<MaybeUninit<Chunk>>(),
            capacity,
            len: 0,
        }
    }

    #[inline(always)]
    fn resize(&mut self, capacity: u32) {
        // SAFETY: Non-null correctly aligned pointer, correct size
        let layout = Layout::for_value(unsafe {
            slice::from_raw_parts(
                self.buffer.as_ptr(),
                1 + (self.capacity as usize).div_ceil(size_of::<Chunk>()),
            )
        });

        // Whole chunks, like in `allocate()` and `drop()`, plus the first one for `strong_count`
        let new_size = if usize::BITS < u64::BITS {
            // With a narrower `usize`, a `u32` capacity can overflow the size or exceed
            // `isize::MAX`, the largest size of an allocation
            let Some(new_size) = (capacity as usize)
                .checked_next_multiple_of(size_of::<Chunk>())
                .and_then(|size| size.checked_add(size_of::<Chunk>()))
                .filter(|&size| Layout::from_size_align(size, align_of::<Chunk>()).is_ok())
            else {
                panic!(
                    "Realloc from {} to {capacity} has failed: too large",
                    self.capacity()
                );
            };
            new_size
        } else {
            size_of::<Chunk>() + (capacity as usize).next_multiple_of(size_of::<Chunk>())
        };

        #[expect(
            clippy::cast_ptr_alignment,
            reason = "Cast from correct alignment to bytes and back due to API requirements"
        )]
        // SAFETY: Allocated with global allocator, correct layout, non-zero size that is a
        // multiple of alignment and at most `isize::MAX` (checked above where a `u32` capacity can
        // exceed it)
        let new_ptr = unsafe {
            realloc(self.buffer.as_ptr().cast::<u8>(), layout, new_size)
                .cast::<MaybeUninit<Chunk>>()
        };
        let Some(new_ptr) = NonNull::new(new_ptr) else {
            panic!("Realloc from {} to {new_size} has failed", self.capacity());
        };

        self.buffer = new_ptr;
        self.capacity = capacity;
    }

    #[inline(always)]
    const fn len(&self) -> u32 {
        self.len
    }

    /// `len` bytes must be initialized
    #[inline(always)]
    unsafe fn set_len(&mut self, len: u32) {
        debug_assert!(
            len <= self.capacity(),
            "Too many bytes {} > {}",
            len,
            self.capacity()
        );
        self.len = len;
    }

    #[inline(always)]
    const fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline(always)]
    const fn strong_count_ref(&self) -> &AtomicUsize {
        // SAFETY: The first chunk is allocated for `strong_count`, which is a correctly aligned
        // copy type initialized in the constructor
        unsafe {
            self.buffer
                .as_ptr()
                .cast::<AtomicUsize>()
                .as_ref_unchecked()
        }
    }

    #[inline(always)]
    const fn as_slice(&self) -> &[u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr().cast::<u8>(), len) }
    }

    #[inline(always)]
    const fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr().cast::<u8>(), len) }
    }

    #[inline(always)]
    const fn as_u128_slice(&self) -> &[u128] {
        let len = self.len() as usize / size_of::<u128>();
        // SAFETY: Not null, correctly aligned (chunks are aligned to at least `u128`), and the
        // first `len` elements are initialized, since they are within `self.len()` bytes, which
        // are initialized (a protected invariant of the implementation)
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline(always)]
    const fn as_ptr(&self) -> *const u128 {
        // SAFETY: Constructor allocates the first chunk for `strong_count`
        unsafe { self.buffer.as_ptr().cast_const().add(1).cast::<u128>() }
    }

    #[inline(always)]
    const fn as_mut_ptr(&mut self) -> *mut u128 {
        // SAFETY: Constructor allocates the first chunk for `strong_count`
        unsafe { self.buffer.as_ptr().add(1).cast::<u128>() }
    }
}

/// Owned aligned buffer for executor purposes.
///
/// See [`SharedAlignedBuffer`] for a version that can be cheaply cloned while reusing the original
/// allocation.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
pub struct OwnedAlignedBuffer {
    inner: InnerBuffer,
}

impl Deref for OwnedAlignedBuffer {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for OwnedAlignedBuffer {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

// SAFETY: Heap-allocated data structure, points to the same memory if moved
unsafe impl StableDeref for OwnedAlignedBuffer {}

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
    ///
    /// # Panics
    /// If the allocation would be larger than `isize::MAX` bytes
    #[inline(always)]
    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            inner: InnerBuffer::allocate(capacity),
        }
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`, or if the allocation would be larger than
    /// `isize::MAX` bytes
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut instance = Self::with_capacity(0);
        instance.copy_from_slice(bytes);
        instance
    }

    #[inline(always)]
    pub const fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline(always)]
    pub const fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner.as_mut_slice()
    }

    /// Contents as `u128`s.
    ///
    /// Only whole `u128`s are included: when the length is not a multiple of `u128` size, the
    /// bytes after the last whole `u128` are not.
    #[inline(always)]
    pub const fn as_u128_slice(&self) -> &[u128] {
        self.inner.as_u128_slice()
    }

    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u128 {
        self.inner.as_ptr()
    }

    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut u128 {
        self.inner.as_mut_ptr()
    }

    #[inline(always)]
    pub fn into_shared(self) -> SharedAlignedBuffer {
        SharedAlignedBuffer { inner: self.inner }
    }

    /// Ensure capacity of the buffer is at least `capacity`.
    ///
    /// Will re-allocate if necessary.
    ///
    /// # Panics
    /// If the allocation would be larger than `isize::MAX` bytes
    #[inline(always)]
    pub fn ensure_capacity(&mut self, capacity: u32) {
        if capacity > self.capacity() {
            self.inner.resize(capacity);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`, or if the allocation would be larger than
    /// `isize::MAX` bytes
    #[inline(always)]
    pub fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        if len > self.capacity() {
            self.inner
                .resize(len.max(self.capacity().saturating_mul(2)));
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by the type system
        unsafe {
            self.as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());

            self.inner.set_len(len);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// Returns `false` if `self.len() + bytes.len()` doesn't fit into `u32`.
    ///
    /// # Panics
    /// If the allocation would be larger than `isize::MAX` bytes
    #[inline(always)]
    #[must_use]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };

        let Some(new_len) = self.len().checked_add(len) else {
            return false;
        };

        if new_len > self.capacity() {
            self.inner
                .resize(new_len.max(self.capacity().saturating_mul(2)));
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by the type system
        unsafe {
            self.as_mut_ptr()
                .cast::<u8>()
                .add(self.len() as usize)
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());

            self.inner.set_len(new_len);
        }

        true
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    #[inline(always)]
    pub const fn len(&self) -> u32 {
        self.inner.len()
    }

    #[inline(always)]
    pub const fn capacity(&self) -> u32 {
        self.inner.capacity()
    }

    /// Set the length of the useful data to a specified value.
    ///
    /// # Safety
    /// There must be `new_len` bytes initialized in the buffer.
    #[inline(always)]
    pub unsafe fn set_len(&mut self, new_len: u32) {
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
#[derive(Debug, Default, Clone)]
pub struct SharedAlignedBuffer {
    inner: InnerBuffer,
}

impl Deref for SharedAlignedBuffer {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

// SAFETY: Heap-allocated data structure, points to the same memory if moved
unsafe impl StableDeref for SharedAlignedBuffer {}
// SAFETY: Inner buffer is exactly the same and points to the same memory after clone
unsafe impl CloneStableDeref for SharedAlignedBuffer {}
// SAFETY: Inner buffer is exactly the same and points to the same memory after clone
unsafe impl CloneableCart for SharedAlignedBuffer {}

impl SharedAlignedBuffer {
    /// Static reference to an empty buffer
    #[inline(always)]
    pub const fn empty_ref() -> &'static Self {
        &EMPTY_SHARED_ALIGNED_BUFFER
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`, or if the allocation would be larger than
    /// `isize::MAX` bytes
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        OwnedAlignedBuffer::from_bytes(bytes).into_shared()
    }

    /// Convert into owned buffer.
    ///
    /// If this is the last shared instance, then allocation will be reused, otherwise a new
    /// allocation will be created.
    #[inline(always)]
    pub fn into_owned(self) -> OwnedAlignedBuffer {
        if self.inner.strong_count_ref().load(Ordering::Acquire) == 1 {
            OwnedAlignedBuffer { inner: self.inner }
        } else {
            OwnedAlignedBuffer::from_bytes(self.as_slice())
        }
    }

    #[inline(always)]
    pub const fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u128 {
        self.inner.as_ptr()
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    #[inline(always)]
    pub const fn len(&self) -> u32 {
        self.inner.len()
    }
}
