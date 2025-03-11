#[cfg(test)]
mod tests;

use ab_contracts_io_type::MAX_ALIGNMENT;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{mem, slice};

#[repr(C, align(16))]
struct AlignedBytes([u8; MAX_ALIGNMENT as usize]);

const _: () = {
    assert!(
        align_of::<AlignedBytes>() == size_of::<AlignedBytes>(),
        "Size and alignment are both 16 bytes"
    );
};

/// Owned aligned buffer for executor purposes.
///
/// See [`SharedAlignedBuffer`] for a version that can be cheaply cloned, while reusing the original
/// allocation.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
pub(super) struct OwnedAlignedBuffer {
    /// Last 4 bytes reserved for [`SharedAlignedBuffer::strong_count()`]
    buffer: NonNull<[MaybeUninit<AlignedBytes>]>,
    len: u32,
}

// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Send for OwnedAlignedBuffer {}
// SAFETY: Heap-allocated memory buffer can be used from any thread
unsafe impl Sync for OwnedAlignedBuffer {}

impl Clone for OwnedAlignedBuffer {
    #[inline]
    fn clone(&self) -> Self {
        let mut buffer = Self::with_capacity(self.capacity());
        buffer.copy_from_slice(self.as_slice());
        buffer
    }
}

impl Drop for OwnedAlignedBuffer {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: Created from `Box` in constructor
        let _ = unsafe { Box::from_non_null(self.buffer) };
    }
}

impl OwnedAlignedBuffer {
    /// Create a new instance with at least specified capacity.
    ///
    /// NOTE: Actual capacity might be larger due to alignment requirements.
    #[inline(always)]
    pub(super) fn with_capacity(capacity: u32) -> Self {
        Self {
            buffer: Self::allocate_buffer(capacity),
            len: 0,
        }
    }

    /// Allocates a new buffer + one [`AlignedBytes`] worth of memory at the beginning for the
    /// `strong_count` in case it is later converted to [`SharedAlignedBuffer`]
    #[inline(always)]
    fn allocate_buffer(capacity: u32) -> NonNull<[MaybeUninit<AlignedBytes>]> {
        Box::into_non_null(Box::new_uninit_slice(
            1 + (capacity as usize).div_ceil(size_of::<AlignedBytes>()),
        ))
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        let mut instance = Self::with_capacity(0);
        instance.copy_from_slice(bytes);
        instance
    }

    #[inline(always)]
    pub(super) fn as_slice(&self) -> &[u8] {
        let len = self.len as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    #[inline(always)]
    pub(super) fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), len) }
    }

    #[inline(always)]
    pub(super) fn as_ptr(&self) -> *const u8 {
        let buffer_ptr = self.buffer.as_ptr().cast_const().cast::<u8>();
        // SAFETY: Constructor allocates the first element for `strong_count`
        unsafe { buffer_ptr.add(size_of::<AlignedBytes>()) }
    }

    #[inline(always)]
    pub(super) fn as_mut_ptr(&mut self) -> *mut u8 {
        let buffer_ptr = self.buffer.as_ptr().cast::<u8>();
        // SAFETY: Constructor allocates the first element for `strong_count`
        unsafe { buffer_ptr.add(size_of::<AlignedBytes>()) }
    }

    #[inline(always)]
    pub(super) fn into_shared(self) -> SharedAlignedBuffer {
        let instance = ManuallyDrop::new(self);
        SharedAlignedBuffer::new_from_buffer_with_len(instance.buffer, instance.len)
    }

    /// Ensure capacity of the buffer is at least `capacity`.
    ///
    /// Will re-allocate if necessary.
    #[inline(always)]
    pub(super) fn ensure_capacity(&mut self, capacity: u32) {
        // `+ size_of::<AlignedBytes>()` for `strong_count`
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
    pub(super) fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes");
        };

        if len > self.capacity() {
            // Drop old buffer
            // SAFETY: Created from `Box` in constructor
            let _ = unsafe { Box::from_non_null(self.buffer) };
            // Allocate new buffer
            self.buffer = Self::allocate_buffer(len);
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by type system
        unsafe {
            self.as_mut_ptr()
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());
        }

        self.len = len;
    }

    #[inline(always)]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    // TODO: Store precomputed capacity and expose pointer to it
    #[inline(always)]
    pub(super) fn capacity(&self) -> u32 {
        // API constraints capacity to `u32`, hence this never truncates, `- 1` due to
        // `strong_count` stored at the beginning of the buffer
        ((self.buffer.len() - 1) * size_of::<AlignedBytes>()) as u32
    }

    /// Set the length of the useful data to specified value.
    ///
    /// # Safety
    /// There must be `new_len` bytes initialized in the buffer.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub(super) unsafe fn set_len(&mut self, new_len: u32) {
        assert!(
            new_len <= self.capacity(),
            "Too many bytes {} > {}",
            new_len,
            self.capacity()
        );
        self.len = new_len;
    }
}

/// Aligned atomic used in static below
#[repr(C, align(16))]
struct ConstStrongCount(AtomicU32);

/// Wrapper to make pointer `Sync`
#[repr(transparent)]
struct SharableConstStrongCountPtr(NonNull<[MaybeUninit<AlignedBytes>]>);
// SAFETY: Statically allocated memory buffer with atomic can be used from any thread
unsafe impl Sync for SharableConstStrongCountPtr {}

static mut CONST_STRONG_COUNT: &mut [ConstStrongCount] = &mut [ConstStrongCount(AtomicU32::new(1))];
/// SAFETY: Size and layout of both `NonNull<[ConstStrongCount]>` and `SharableConstStrongCountPtr`
/// is the same, `CONST_STRONG_COUNT` static is only mutated through atomic operations
static EMPTY_SHARED_ALIGNED_BUFFER_BUFFER: SharableConstStrongCountPtr = unsafe {
    mem::transmute::<NonNull<[ConstStrongCount]>, SharableConstStrongCountPtr>(NonNull::from_mut(
        CONST_STRONG_COUNT,
    ))
};
static EMPTY_SHARED_ALIGNED_BUFFER: SharedAlignedBuffer = SharedAlignedBuffer {
    buffer: EMPTY_SHARED_ALIGNED_BUFFER_BUFFER.0,
    len: 0,
};

/// Shared aligned buffer for executor purposes.
///
/// See [`OwnedAlignedBuffer`] for a version that can be mutated.
///
/// Data is aligned to 16 bytes (128 bits), which is the largest alignment required by primitive
/// types and by extension any type that implements `TrivialType`/`IoType`.
///
/// NOTE: Counter for number of shared instances is `u32` and will wrap around if exceeded breaking
/// internal invariants (which is extremely unlikely, but still).
#[derive(Debug)]
pub(super) struct SharedAlignedBuffer {
    buffer: NonNull<[MaybeUninit<AlignedBytes>]>,
    len: u32,
}

impl Default for SharedAlignedBuffer {
    #[inline]
    fn default() -> Self {
        OwnedAlignedBuffer::with_capacity(0).into_shared()
    }
}

impl Clone for SharedAlignedBuffer {
    #[inline(always)]
    fn clone(&self) -> Self {
        self.strong_count().fetch_add(1, Ordering::AcqRel);

        Self {
            buffer: self.buffer,
            len: self.len,
        }
    }
}

impl Drop for SharedAlignedBuffer {
    #[inline]
    fn drop(&mut self) {
        if self.strong_count().fetch_sub(1, Ordering::AcqRel) == 1 {
            // SAFETY: Created from `Box` in constructor
            let _ = unsafe { Box::from_non_null(self.buffer) };
        }
    }
}

// SAFETY: Heap-allocated memory buffer and atomic can be used from any thread
unsafe impl Send for SharedAlignedBuffer {}
// SAFETY: Heap-allocated memory buffer and atomic can be used from any thread
unsafe impl Sync for SharedAlignedBuffer {}

impl SharedAlignedBuffer {
    /// # Safety
    /// Must only be called with First bytes are allocated to be `strong_count`
    fn new_from_buffer_with_len(buffer: NonNull<[MaybeUninit<AlignedBytes>]>, len: u32) -> Self {
        // SAFETY: The first bytes are allocated for strong count, which is a correctly aligned copy
        // type
        unsafe { buffer.as_ptr().cast::<AtomicU32>().write(AtomicU32::new(1)) };
        Self { buffer, len }
    }

    /// Static reference to an empty buffer
    #[inline(always)]
    pub(super) fn empty_ref() -> &'static Self {
        &EMPTY_SHARED_ALIGNED_BUFFER
    }

    /// Create a new instance from provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` doesn't fit into `u32`
    #[inline(always)]
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        OwnedAlignedBuffer::from_bytes(bytes).into_shared()
    }

    /// Convert into owned buffer.
    ///
    /// If this is the last shared instance, then allocation will be reused, otherwise new
    /// allocation will be created.
    ///
    /// Returns `None` if there exit other shared instances.
    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    #[inline(always)]
    pub(super) fn into_owned(self) -> OwnedAlignedBuffer {
        let instance = ManuallyDrop::new(self);
        if instance.strong_count().fetch_sub(1, Ordering::AcqRel) == 1 {
            OwnedAlignedBuffer {
                buffer: instance.buffer,
                len: instance.len,
            }
        } else {
            let mut owned_instance = OwnedAlignedBuffer::with_capacity(0);
            owned_instance.copy_from_slice(instance.as_slice());
            owned_instance
        }
    }

    #[inline(always)]
    pub(super) fn as_slice(&self) -> &[u8] {
        // SAFETY: Not null and size is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len as usize) }
    }

    #[inline(always)]
    pub(super) fn as_ptr(&self) -> *const u8 {
        let buffer_ptr = self.buffer.as_ptr().cast_const().cast::<u8>();
        // SAFETY: Constructor allocates the first element for `strong_count`
        unsafe { buffer_ptr.add(size_of::<AlignedBytes>()) }
    }

    #[inline(always)]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    #[inline(always)]
    fn strong_count(&self) -> &AtomicU32 {
        // SAFETY: The first bytes are allocated for strong count, which is a correctly aligned copy
        // type initialized in constructor if `true`
        unsafe { self.buffer.as_ptr().cast::<AtomicU32>().as_ref_unchecked() }
    }
}
