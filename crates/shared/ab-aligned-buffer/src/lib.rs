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

use alloc::alloc::{alloc, dealloc, handle_alloc_error, realloc};
use core::alloc::Layout;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering, fence};
use stable_deref_trait::{CloneStableDeref, StableDeref};
use yoke::CloneableCart;

/// Alignment of the contents of every buffer
const ALIGNMENT: usize = 16;

/// Header of every allocation, stored right before the contents of the buffer
#[repr(C, align(16))]
struct Header {
    /// The number of shared buffers referencing the allocation, owned buffers don't use it
    strong_count: AtomicUsize,
    /// Capacity of the buffer in bytes
    capacity: u32,
}

const _: () = {
    assert!(size_of::<Header>() == ALIGNMENT);
    assert!(align_of::<Header>() == ALIGNMENT);
    // Buffers are accessed as `u128`s
    assert!(ALIGNMENT >= align_of::<u128>());
};

const _: () = {
    // The size of an allocation for the largest capacity, computed like in `layout()`, is valid
    let Ok(_) = Layout::from_size_align(
        size_of::<Header>()
            + (OwnedAlignedBuffer::MAX_CAPACITY as usize).next_multiple_of(ALIGNMENT),
        ALIGNMENT,
    ) else {
        panic!("The largest capacity can't be allocated");
    };
    // A larger capacity either doesn't fit into `u32` or its allocation is too large
    assert!(
        OwnedAlignedBuffer::MAX_CAPACITY == u32::MAX
            || Layout::from_size_align(
                size_of::<Header>()
                    + (OwnedAlignedBuffer::MAX_CAPACITY as usize + 1).next_multiple_of(ALIGNMENT),
                ALIGNMENT,
            )
            .is_err()
    );
};

/// Header of empty buffers, which are not backed by an allocation.
///
/// Empty owned buffers have zero capacity, empty shared buffers don't hold a reference, so nothing
/// ever modifies it.
static EMPTY_HEADER: Header = Header {
    strong_count: AtomicUsize::new(0),
    capacity: 0,
};

static EMPTY_SHARED_ALIGNED_BUFFER: SharedAlignedBuffer = SharedAlignedBuffer {
    inner: InnerBuffer::EMPTY,
};

#[derive(Debug)]
struct InnerBuffer {
    /// The first byte of the contents, the header is right before it
    buffer: NonNull<u8>,
    len: u32,
}

// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Send for InnerBuffer {}
// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Sync for InnerBuffer {}

impl InnerBuffer {
    /// Empty buffer that is not backed by an allocation
    const EMPTY: Self = Self {
        // SAFETY: One byte past the end of the header, which is the same as for allocated buffers
        buffer: unsafe {
            NonNull::from_ref(&EMPTY_HEADER)
                .cast::<u8>()
                .byte_add(size_of::<Header>())
        },
        len: 0,
    };

    /// Layout of an allocation for a specified capacity.
    ///
    /// Returns `None` if the allocation would be too large.
    #[inline(always)]
    fn layout(capacity: u32) -> Option<Layout> {
        // With a narrower `usize`, a `u32` capacity can overflow the size or exceed `isize::MAX`,
        // the largest size of an allocation
        let size = (capacity as usize)
            .checked_next_multiple_of(ALIGNMENT)?
            .checked_add(size_of::<Header>())?;
        Layout::from_size_align(size, ALIGNMENT).ok()
    }

    /// Layout of the allocation that backs the buffer.
    ///
    /// # Safety
    /// The buffer must be backed by an allocation (have non-zero capacity).
    #[inline(always)]
    unsafe fn allocated_layout(&self) -> Layout {
        let size = size_of::<Header>() + (self.capacity() as usize).next_multiple_of(ALIGNMENT);
        // SAFETY: Allocated with the same layout before, which was checked by `Self::layout()`
        unsafe { Layout::from_size_align_unchecked(size, ALIGNMENT) }
    }

    /// Allocates a new buffer with a header at the beginning.
    ///
    /// Zero capacity results in an empty buffer that is not backed by an allocation.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or allocation fails
    #[inline(always)]
    fn allocate(capacity: u32) -> Self {
        if capacity == 0 {
            return Self::EMPTY;
        }

        let Some(layout) = Self::layout(capacity) else {
            panic!("Allocation of capacity {capacity} has failed: too large");
        };
        // SAFETY: Non-zero size
        let header = unsafe { alloc(layout) };
        #[expect(
            clippy::cast_ptr_alignment,
            reason = "Allocated with alignment of the header"
        )]
        let Some(header) = NonNull::new(header.cast::<Header>()) else {
            handle_alloc_error(layout);
        };
        // SAFETY: Allocation starts with a correctly aligned header
        unsafe {
            header.write(Header {
                strong_count: AtomicUsize::new(1),
                capacity,
            });
        }

        Self {
            // SAFETY: Contents are right after the header within the same allocation
            buffer: unsafe { header.add(1).cast::<u8>() },
            len: 0,
        }
    }

    /// Changes capacity of the buffer, the buffer must not be shared.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or reallocation fails
    #[inline(always)]
    fn resize(&mut self, capacity: u32) {
        let old_capacity = self.capacity();
        if old_capacity == 0 {
            // Empty buffer is not backed by an allocation, and it has no contents to preserve
            *self = Self::allocate(capacity);
            return;
        }

        let Some(new_layout) = Self::layout(capacity) else {
            panic!("Realloc from {old_capacity} to {capacity} has failed: too large");
        };
        // SAFETY: Non-zero capacity checked above
        let old_layout = unsafe { self.allocated_layout() };

        // SAFETY: Allocated with global allocator and `old_layout`, non-zero new size that is at
        // most `isize::MAX` when rounded up to the alignment (checked by `Layout` above)
        let header = unsafe {
            realloc(
                self.header_ptr().as_ptr().cast::<u8>(),
                old_layout,
                new_layout.size(),
            )
        };
        #[expect(
            clippy::cast_ptr_alignment,
            reason = "Reallocated with alignment of the header"
        )]
        let Some(header) = NonNull::new(header.cast::<Header>()) else {
            handle_alloc_error(new_layout);
        };
        // SAFETY: Reallocation preserved the header at the beginning, and the buffer is not shared
        unsafe {
            (*header.as_ptr()).capacity = capacity;
        }

        // SAFETY: Contents are right after the header within the same allocation
        self.buffer = unsafe { header.add(1).cast::<u8>() };
    }

    /// Deallocates the buffer.
    ///
    /// # Safety
    /// The buffer must be backed by an allocation (have non-zero capacity) and not be referenced
    /// anymore.
    #[inline(always)]
    unsafe fn deallocate(&self) {
        // SAFETY: Allocated with global allocator and this layout, guaranteed by the caller to be
        // backed by an allocation that is not referenced anymore
        unsafe {
            dealloc(
                self.header_ptr().as_ptr().cast::<u8>(),
                self.allocated_layout(),
            );
        }
    }

    #[inline(always)]
    const fn header_ptr(&self) -> NonNull<Header> {
        // SAFETY: The header is right before the contents
        unsafe { self.buffer.cast::<Header>().sub(1) }
    }

    #[inline(always)]
    const fn header(&self) -> &Header {
        // SAFETY: The header is initialized in the constructor and lives as long as the buffer
        unsafe { self.header_ptr().as_ref() }
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
        self.header().capacity
    }

    #[inline(always)]
    const fn strong_count_ref(&self) -> &AtomicUsize {
        &self.header().strong_count
    }

    #[inline(always)]
    const fn as_slice(&self) -> &[u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.buffer.as_ptr().cast_const(), len) }
    }

    #[inline(always)]
    const fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.buffer.as_ptr(), len) }
    }

    #[inline(always)]
    const fn as_u128_slice(&self) -> &[u128] {
        let len = self.len() as usize / size_of::<u128>();
        // SAFETY: Not null, correctly aligned (contents are aligned to at least `u128`), and the
        // first `len` elements are initialized, since they are within `self.len()` bytes, which
        // are initialized (a protected invariant of the implementation)
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Contents are aligned to `ALIGNMENT`"
    )]
    const fn as_ptr(&self) -> *const u128 {
        self.buffer.as_ptr().cast_const().cast::<u128>()
    }

    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Contents are aligned to `ALIGNMENT`"
    )]
    const fn as_mut_ptr(&mut self) -> *mut u128 {
        self.buffer.as_ptr().cast::<u128>()
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

impl Drop for OwnedAlignedBuffer {
    #[inline(always)]
    fn drop(&mut self) {
        // Owned buffer is not shared, so the reference counter is not used, and an empty buffer
        // with zero capacity is not backed by an allocation
        if self.inner.capacity() != 0 {
            // SAFETY: Backed by an allocation and not shared
            unsafe {
                self.inner.deallocate();
            }
        }
    }
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
        Self::from_bytes(self.as_slice())
    }
}

impl OwnedAlignedBuffer {
    /// The largest capacity of a buffer.
    ///
    /// It is `u32::MAX` where `usize` is 64-bit, and smaller where `usize` is 32-bit, since an
    /// allocation can't be larger than `isize::MAX` bytes. The allocator may still fail to allocate
    /// a buffer that large.
    pub const MAX_CAPACITY: u32 = if usize::BITS < u64::BITS {
        // The header is in front of the contents
        ((isize::MAX.cast_unsigned() / ALIGNMENT - 1) * ALIGNMENT) as u32
    } else {
        u32::MAX
    };

    /// Create a new instance with at least specified capacity.
    ///
    /// NOTE: Actual capacity might be larger due to alignment requirements.
    ///
    /// # Panics
    /// If `capacity` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            inner: InnerBuffer::allocate(capacity),
        }
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        let mut instance = Self::with_capacity(len);
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

    /// Convert into shared buffer.
    ///
    /// Allocation is reused, except for empty buffer, which doesn't need an allocation.
    #[inline(always)]
    pub fn into_shared(self) -> SharedAlignedBuffer {
        if self.is_empty() {
            return SharedAlignedBuffer::default();
        }

        let this = ManuallyDrop::new(self);
        SharedAlignedBuffer {
            // SAFETY: Moved out of `ManuallyDrop` that is never used again, the reference counter
            // is `1` since owned buffer is not shared
            inner: unsafe { (&raw const this.inner).read() },
        }
    }

    /// Ensure capacity of the buffer is at least `capacity`.
    ///
    /// Will re-allocate if necessary.
    ///
    /// # Panics
    /// If `capacity` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn ensure_capacity(&mut self, capacity: u32) {
        if capacity > self.capacity() {
            self.inner.resize(capacity);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    #[cfg_attr(
        target_pointer_width = "64",
        expect(
            clippy::unnecessary_min_or_max,
            reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
        )
    )]
    pub fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        if len > self.capacity() {
            // Doubling the capacity is capped by the largest one `Layout` allows
            self.inner
                .resize(len.max(self.capacity().saturating_mul(2).min(Self::MAX_CAPACITY)));
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
    /// Returns `false` and leaves the buffer unchanged if `self.len() + bytes.len()` is larger than
    /// [`Self::MAX_CAPACITY`].
    ///
    /// # Panics
    /// If memory re-allocation fails
    #[inline(always)]
    #[must_use = "Check for overflow"]
    #[cfg_attr(
        target_pointer_width = "64",
        expect(
            clippy::absurd_extreme_comparisons,
            clippy::unnecessary_min_or_max,
            reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
        )
    )]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };

        let Some(new_len) = self.len().checked_add(len) else {
            return false;
        };
        if new_len > Self::MAX_CAPACITY {
            return false;
        }

        if new_len > self.capacity() {
            // Doubling the capacity is capped by the largest one `Layout` allows
            self.inner
                .resize(new_len.max(self.capacity().saturating_mul(2).min(Self::MAX_CAPACITY)));
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
#[derive(Debug)]
pub struct SharedAlignedBuffer {
    /// Empty buffer doesn't hold a reference to an allocation and points to the static empty
    /// header instead, all other buffers hold one reference
    inner: InnerBuffer,
}

impl Default for SharedAlignedBuffer {
    #[inline(always)]
    fn default() -> Self {
        Self {
            inner: InnerBuffer::EMPTY,
        }
    }
}

impl Clone for SharedAlignedBuffer {
    #[inline(always)]
    fn clone(&self) -> Self {
        if !self.is_empty() {
            let strong_count = self.inner.strong_count_ref();
            let old_count = strong_count.fetch_add(1, Ordering::Relaxed);
            // Leaked clones could wrap a counter narrower than 64 bits around, and a drop would
            // then free the buffer while other clones still use it
            if usize::BITS < u64::BITS && old_count > isize::MAX.cast_unsigned() {
                strong_count.fetch_sub(1, Ordering::Relaxed);
                panic!("Too many clones of a shared aligned buffer");
            }
        }

        Self {
            inner: InnerBuffer {
                buffer: self.inner.buffer,
                len: self.inner.len,
            },
        }
    }
}

impl Drop for SharedAlignedBuffer {
    #[inline(always)]
    fn drop(&mut self) {
        if !self.is_empty()
            && self
                .inner
                .strong_count_ref()
                .fetch_sub(1, Ordering::Release)
                == 1
        {
            // Synchronizes with decrements in other instances, such that their accesses to the
            // contents happen before deallocation
            fence(Ordering::Acquire);
            // SAFETY: Non-empty buffer is backed by an allocation, and this was the last reference
            unsafe {
                self.inner.deallocate();
            }
        }
    }
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
    /// If `bytes.len()` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
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
        if self.is_empty() {
            return OwnedAlignedBuffer::with_capacity(0);
        }

        // Synchronizes with decrements in other instances, such that their accesses to the
        // contents happen before modifications through the owned buffer
        if self.inner.strong_count_ref().load(Ordering::Acquire) == 1 {
            let this = ManuallyDrop::new(self);
            OwnedAlignedBuffer {
                // SAFETY: Moved out of `ManuallyDrop` that is never used again, this was the last
                // reference, so owned buffer is not shared
                inner: unsafe { (&raw const this.inner).read() },
            }
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

    #[inline(always)]
    pub const fn capacity(&self) -> u32 {
        self.inner.capacity()
    }
}
