//! Efficient abstraction for aligned memory buffers with both owned and shared variants.
//!
//! [`OwnedAlignedBuffer`] represents an aligned memory location that can be modified.
//!
//! [`SharedAlignedBuffer`] can't be modified but supports cheap reference-counting clones (like
//! `Arc`, but much more efficient).
//!
//! Contents of buffers are aligned to 16 bytes by default, which is the largest alignment required
//! by primitive types. Alignment of the contents is a part of the type of the buffer (see
//! [`Alignment`]). Allocations can be aligned more strictly than that, for example, to 4096 bytes
//! for direct I/O.
//!
//! Does not require a standard library (`no_std`) but does require allocator and atomics.

#![cfg_attr(test, feature(pointer_is_aligned_to))]
#![no_std]

pub mod alignment;
mod sealed;
#[cfg(test)]
mod tests;

extern crate alloc;

use crate::alignment::{Align16, Align4096, Alignment, AtLeast};
use alloc::alloc::{alloc, dealloc, handle_alloc_error, realloc};
use core::alloc::Layout;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};
use core::{hint, ptr, slice};
use stable_deref_trait::{CloneStableDeref, StableDeref};
use yoke::CloneableCart;

/// The smallest alignment of allocations, which is also the size of the header
const MIN_ALLOCATION_ALIGNMENT: usize = Align16::ALIGN;
/// The strictest supported alignment of allocations
const MAX_ALIGNMENT: usize = Align4096::ALIGN;
/// See [`OwnedAlignedBuffer::MAX_CAPACITY`]
const MAX_CAPACITY: u32 = if usize::BITS < u64::BITS {
    // Space for the header is reserved in front of the contents, which is as large as the
    // alignment
    ((isize::MAX.cast_unsigned() / MAX_ALIGNMENT - 1) * MAX_ALIGNMENT) as u32
} else {
    u32::MAX
};

/// Header of every allocation, stored right before the contents of the buffer
#[repr(C, align(16))]
struct Header {
    /// The number of shared buffers referencing the allocation, owned buffers don't use it
    strong_count: AtomicUsize,
    /// Capacity of the buffer in bytes
    capacity: u32,
    /// Base 2 logarithm of the alignment of the allocation
    alignment_log2: u32,
}

const _: () = {
    assert!(size_of::<Header>() == MIN_ALLOCATION_ALIGNMENT);
    assert!(align_of::<Header>() == MIN_ALLOCATION_ALIGNMENT);
    // Contents of buffers aligned to `Align16` are accessed as `u128`s
    assert!(Align16::ALIGN >= align_of::<u128>());
};

const _: () = {
    // The size of an allocation for the largest capacity and alignment, computed like in
    // `InnerBuffer::layout()`, is valid
    let Ok(_) = Layout::from_size_align(
        MAX_ALIGNMENT + (MAX_CAPACITY as usize).next_multiple_of(MAX_ALIGNMENT),
        MAX_ALIGNMENT,
    ) else {
        panic!("The largest capacity can't be allocated");
    };
    // A larger capacity either doesn't fit into `u32` or its allocation is too large
    assert!(
        MAX_CAPACITY == u32::MAX
            || Layout::from_size_align(
                MAX_ALIGNMENT + (MAX_CAPACITY as usize + 1).next_multiple_of(MAX_ALIGNMENT),
                MAX_ALIGNMENT,
            )
            .is_err()
    );
};

/// Memory of empty buffers, which are not backed by an allocation.
///
/// Empty buffers point to the end of it, right after the header, which makes them aligned to any
/// supported alignment. Empty owned buffers have zero capacity and empty shared buffers don't hold
/// a reference, so nothing ever modifies it.
#[repr(C, align(4096))]
struct EmptyAllocation {
    padding: [u8; MAX_ALIGNMENT - size_of::<Header>()],
    header: Header,
}

const _: () = {
    assert!(align_of::<EmptyAllocation>() == MAX_ALIGNMENT);
    assert!(size_of::<EmptyAllocation>() == MAX_ALIGNMENT);
};

static EMPTY_ALLOCATION: EmptyAllocation = EmptyAllocation {
    padding: [0; _],
    header: Header {
        strong_count: AtomicUsize::new(0),
        capacity: 0,
        alignment_log2: MAX_ALIGNMENT.ilog2(),
    },
};

/// Empty buffer referenced by [`SharedAlignedBuffer::empty_ref()`] for any alignment
static EMPTY_INNER_BUFFER: InnerBuffer = InnerBuffer::EMPTY;

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
        // SAFETY: Right after the header at the end of the empty allocation, which is the same as
        // for allocated buffers
        buffer: unsafe {
            NonNull::from_ref(&EMPTY_ALLOCATION)
                .cast::<u8>()
                .byte_add(size_of::<EmptyAllocation>())
        },
        len: 0,
    };

    /// Layout of an allocation for a specified capacity and alignment.
    ///
    /// The header is in front of the contents, and the size of the allocation is a multiple of
    /// alignment, which allocators handle most efficiently.
    ///
    /// Returns `None` if the capacity is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or the
    /// allocation would be too large.
    #[inline(always)]
    #[cfg_attr(
        target_pointer_width = "64",
        expect(
            clippy::absurd_extreme_comparisons,
            reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
        )
    )]
    fn layout(capacity: u32, alignment: usize) -> Option<Layout> {
        // The largest capacity is the same for all alignments, even if allocations with smaller
        // alignment could be slightly larger
        if capacity > MAX_CAPACITY {
            return None;
        }
        // With a narrower `usize`, a `u32` capacity can overflow the size or exceed `isize::MAX`,
        // the largest size of an allocation
        let size = (capacity as usize)
            .checked_next_multiple_of(alignment)?
            .checked_add(alignment)?;
        Layout::from_size_align(size, alignment).ok()
    }

    /// Layout of the allocation that backs the buffer.
    ///
    /// # Safety
    /// The buffer must be backed by an allocation (have non-zero capacity).
    #[inline(always)]
    unsafe fn allocated_layout(&self) -> Layout {
        let alignment = self.alignment();
        let size = alignment + (self.capacity() as usize).next_multiple_of(alignment);
        // SAFETY: Allocated with the same layout before, which was checked by `Self::layout()`
        unsafe { Layout::from_size_align_unchecked(size, alignment) }
    }

    /// Allocates a new buffer with a header in front of the contents.
    ///
    /// `alignment` must be a supported alignment that is not smaller than
    /// `MIN_ALLOCATION_ALIGNMENT`. Zero capacity results in an empty buffer that is not backed by
    /// an allocation.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or allocation fails
    #[inline(always)]
    fn allocate(capacity: u32, alignment: usize) -> Self {
        debug_assert!(
            alignment.is_power_of_two()
                && (MIN_ALLOCATION_ALIGNMENT..=MAX_ALIGNMENT).contains(&alignment),
            "Unsupported alignment {alignment}"
        );

        if capacity == 0 {
            return Self::EMPTY;
        }

        let Some(layout) = Self::layout(capacity, alignment) else {
            panic!("Allocation of capacity {capacity} has failed: too large");
        };
        // SAFETY: Non-zero size
        let allocation = unsafe { alloc(layout) };
        let Some(allocation) = NonNull::new(allocation) else {
            handle_alloc_error(layout);
        };
        // SAFETY: Contents start right after the space reserved for the header at the beginning of
        // the allocation, which is as large as the alignment
        let buffer = unsafe { allocation.byte_add(alignment) };

        // SAFETY: The header is right before the contents and is correctly aligned, since the
        // contents are aligned to at least the size of the header
        unsafe {
            buffer.cast::<Header>().sub(1).write(Header {
                strong_count: AtomicUsize::new(1),
                capacity,
                alignment_log2: alignment.ilog2(),
            });
        }

        Self { buffer, len: 0 }
    }

    /// Changes capacity of the buffer while preserving its contents, the buffer must not be
    /// shared.
    ///
    /// The new allocation is aligned to at least `min_alignment`, which must be a supported
    /// alignment.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or allocation fails
    #[inline(always)]
    fn resize(&mut self, capacity: u32, min_alignment: usize) {
        let old_capacity = self.capacity();
        if old_capacity == 0 {
            // Empty buffer is not backed by an allocation, and it has no contents to preserve
            *self = Self::allocate(capacity, min_alignment.max(MIN_ALLOCATION_ALIGNMENT));
            return;
        }

        let alignment = self.alignment();
        if alignment != MIN_ALLOCATION_ALIGNMENT || min_alignment > MIN_ALLOCATION_ALIGNMENT {
            // Copy just the contents into a new allocation
            let mut new = Self::allocate(capacity, alignment.max(min_alignment));
            // SAFETY: Different allocations, `len` bytes are initialized and fit into the new
            // capacity, since owned buffers only grow
            unsafe {
                new.buffer
                    .copy_from_nonoverlapping(self.buffer, self.len as usize);
            }
            new.len = self.len;
            // SAFETY: Backed by an allocation and not shared
            unsafe {
                self.deallocate();
            }
            *self = new;
            return;
        }

        let Some(new_layout) = Self::layout(capacity, alignment) else {
            panic!("Realloc from {old_capacity} to {capacity} has failed: too large");
        };
        // SAFETY: Non-zero capacity checked above
        let old_layout = unsafe { self.allocated_layout() };

        // SAFETY: Allocated with global allocator and `old_layout`, non-zero new size that is at
        // most `isize::MAX` when rounded up to the alignment (checked by `Layout` above)
        let allocation = unsafe {
            realloc(
                self.allocation_ptr().as_ptr(),
                old_layout,
                new_layout.size(),
            )
        };
        let Some(allocation) = NonNull::new(allocation) else {
            handle_alloc_error(new_layout);
        };
        // SAFETY: Contents start right after the space reserved for the header at the beginning of
        // the allocation, which is as large as the alignment
        self.buffer = unsafe { allocation.byte_add(alignment) };
        // SAFETY: Reallocation preserved the header, and the buffer is not shared
        unsafe {
            (*self.header_ptr().as_ptr()).capacity = capacity;
        }
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
            dealloc(self.allocation_ptr().as_ptr(), self.allocated_layout());
        }
    }

    /// Creates an owned copy of the contents aligned to at least `min_alignment`, which must be a
    /// supported alignment
    #[inline(always)]
    fn copy(&self, min_alignment: usize) -> Self {
        let mut new = Self::allocate(
            self.len,
            self.alignment()
                .max(min_alignment)
                .max(MIN_ALLOCATION_ALIGNMENT),
        );
        // SAFETY: Different allocations, `len` bytes are initialized and fit into the new capacity
        unsafe {
            new.buffer
                .copy_from_nonoverlapping(self.buffer, self.len as usize);
        }
        new.len = self.len;
        new
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
    fn allocation_ptr(&self) -> NonNull<u8> {
        // SAFETY: Contents start right after the space reserved for the header at the beginning of
        // the allocation, which is as large as the alignment
        unsafe { self.buffer.byte_sub(self.alignment()) }
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

    /// Alignment of the allocation, or the alignment an empty buffer without an allocation grows
    /// into for contents aligned to `content_alignment`
    #[inline(always)]
    fn allocation_alignment(&self, content_alignment: usize) -> usize {
        if ptr::eq(self.header(), &raw const EMPTY_ALLOCATION.header) {
            content_alignment.max(MIN_ALLOCATION_ALIGNMENT)
        } else {
            self.alignment()
        }
    }

    #[inline(always)]
    const fn alignment(&self) -> usize {
        let alignment = 2_usize.pow(self.header().alignment_log2);
        // SAFETY: The header stores the binary logarithm of a supported alignment, which is never
        // larger than `MAX_ALIGNMENT`
        unsafe { hint::assert_unchecked(alignment != 0) }
        alignment
    }

    #[inline(always)]
    const fn strong_count_ref(&self) -> &AtomicUsize {
        &self.header().strong_count
    }

    #[inline(always)]
    const fn as_slice(&self) -> &[u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline(always)]
    const fn as_mut_slice(&mut self) -> &mut [u8] {
        let len = self.len() as usize;
        // SAFETY: Not null and length is a protected invariant of the implementation
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), len) }
    }

    #[inline(always)]
    const fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr().cast_const()
    }

    #[inline(always)]
    const fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.as_ptr()
    }

    #[inline(always)]
    fn is_aligned_to(&self, alignment: usize) -> bool {
        self.buffer.addr().get().is_multiple_of(alignment)
    }
}

/// Owned aligned buffer for executor purposes.
///
/// See [`SharedAlignedBuffer`] for a version that can be cheaply cloned while reusing the original
/// allocation.
///
/// Contents are aligned to `A`, by default 16 bytes (128 bits), which is the largest alignment
/// required by primitive types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
pub struct OwnedAlignedBuffer<A = Align16>
where
    A: Alignment,
{
    inner: InnerBuffer,
    alignment: PhantomData<A>,
}

impl<A> Drop for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
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

impl<A> Deref for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<A> DerefMut for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

// SAFETY: Heap-allocated data structure, points to the same memory if moved
unsafe impl<A> StableDeref for OwnedAlignedBuffer<A> where A: Alignment {}

impl<A> Default for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    #[inline(always)]
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<A> Clone for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    /// Allocates as much as the length of the buffer with the same alignment
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::from_inner(self.inner.copy(A::ALIGN))
    }
}

impl<A> OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    /// The largest capacity of a buffer.
    ///
    /// It is `u32::MAX` where `usize` is 64-bit, and smaller where `usize` is 32-bit, since an
    /// allocation can't be larger than `isize::MAX` bytes. The allocator may still fail to allocate
    /// a buffer that large.
    pub const MAX_CAPACITY: u32 = MAX_CAPACITY;

    /// Create a new instance with specified capacity.
    ///
    /// Allocation is aligned to the alignment of the contents, but at least 16 bytes.
    ///
    /// # Panics
    /// If `capacity` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn with_capacity(capacity: u32) -> Self {
        Self::from_inner(InnerBuffer::allocate(
            capacity,
            A::ALIGN.max(MIN_ALLOCATION_ALIGNMENT),
        ))
    }

    /// Create a new instance from provided bytes.
    ///
    /// Allocation is aligned to the alignment of the contents, but at least 16 bytes.
    ///
    /// # Panics
    /// If `bytes.len()` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        let mut instance = Self::with_capacity(len);
        instance.copy_from_slice(bytes);
        instance
    }

    /// Empty buffer that is not backed by an allocation
    pub const EMPTY: Self = Self::from_inner(InnerBuffer::EMPTY);

    #[inline(always)]
    const fn from_inner(inner: InnerBuffer) -> Self {
        Self {
            inner,
            alignment: PhantomData,
        }
    }

    #[inline(always)]
    fn into_inner(self) -> InnerBuffer {
        let this = ManuallyDrop::new(self);
        // SAFETY: Moved out of `ManuallyDrop` that is never used again
        unsafe { (&raw const this.inner).read() }
    }

    /// Create a new instance with specified capacity and allocation aligned to `AA`.
    ///
    /// Zero capacity doesn't allocate, and such a buffer grows into an allocation aligned to the
    /// alignment of the contents, but at least 16 bytes.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn with_capacity_aligned<AA>(capacity: u32) -> Self
    where
        AA: AtLeast<A> + AtLeast<Align16>,
    {
        Self::from_inner(InnerBuffer::allocate(capacity, AA::ALIGN))
    }

    #[inline(always)]
    pub const fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    #[inline(always)]
    pub const fn as_mut_slice(&mut self) -> &mut [u8] {
        self.inner.as_mut_slice()
    }

    /// Pointer to the first byte of the contents
    #[inline(always)]
    pub const fn as_byte_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    /// Mutable pointer to the first byte of the contents
    #[inline(always)]
    pub const fn as_mut_byte_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }

    /// Convert into shared buffer.
    ///
    /// Allocation is reused, except for empty buffer, which doesn't need an allocation.
    #[inline(always)]
    pub fn into_shared(self) -> SharedAlignedBuffer<A> {
        if self.is_empty() {
            return SharedAlignedBuffer::EMPTY;
        }

        // The reference counter is `1` since owned buffer is not shared
        SharedAlignedBuffer::from_inner(self.into_inner())
    }

    /// Convert into a buffer with a weaker alignment of the contents, which is free.
    ///
    /// Conversion to a stricter alignment doesn't compile, see [`Self::try_into_start_align()`]
    /// instead:
    /// ```compile_fail
    /// use ab_aligned_buffer::OwnedAlignedBuffer;
    /// use ab_aligned_buffer::alignment::Align4096;
    ///
    /// // Contents aligned to 16 bytes are not necessarily aligned to 4096 bytes
    /// let _ = OwnedAlignedBuffer::with_capacity(1).into_start_align::<Align4096>();
    /// ```
    #[inline(always)]
    #[must_use = "The buffer is dropped otherwise"]
    pub fn into_start_align<B>(self) -> OwnedAlignedBuffer<B>
    where
        A: AtLeast<B>,
        B: Alignment,
    {
        OwnedAlignedBuffer::from_inner(self.into_inner())
    }

    /// Convert into a buffer with a different alignment of the contents, which is free.
    ///
    /// Returns the buffer back if its contents are not aligned to `B`.
    #[inline(always)]
    pub fn try_into_start_align<B>(self) -> Result<OwnedAlignedBuffer<B>, Self>
    where
        B: Alignment,
    {
        if self.inner.is_aligned_to(B::ALIGN) {
            Ok(OwnedAlignedBuffer::from_inner(self.into_inner()))
        } else {
            Err(self)
        }
    }

    /// Ensure capacity of the buffer is at least `capacity`.
    ///
    /// Will re-allocate if necessary.
    ///
    /// # Panics
    /// If `capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn ensure_capacity(&mut self, capacity: u32) {
        if capacity > self.capacity() {
            self.inner.resize(capacity, A::ALIGN);
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
            self.inner.resize(
                len.max(self.capacity().saturating_mul(2).min(MAX_CAPACITY)),
                A::ALIGN,
            );
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by the type system
        unsafe {
            self.as_mut_byte_ptr()
                .copy_from_nonoverlapping(bytes.as_ptr(), bytes.len());

            self.inner.set_len(len);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// Returns `false` and leaves the buffer unchanged if `self.len() + bytes.len()` is larger than
    /// [`OwnedAlignedBuffer::MAX_CAPACITY`].
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
        if new_len > MAX_CAPACITY {
            return false;
        }

        if new_len > self.capacity() {
            // Doubling the capacity is capped by the largest one `Layout` allows
            self.inner.resize(
                new_len.max(self.capacity().saturating_mul(2).min(MAX_CAPACITY)),
                A::ALIGN,
            );
        }

        // SAFETY: Sufficient capacity guaranteed above, natural alignment of bytes is 1 for input
        // and output, non-overlapping allocations guaranteed by the type system
        unsafe {
            self.as_mut_byte_ptr()
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

    /// Alignment of the allocation in bytes.
    ///
    /// It is not smaller than the alignment of the contents, unless the contents happen to be
    /// aligned more strictly than the allocation (see [`Self::try_into_start_align()`]). Empty
    /// buffer without an allocation reports the alignment it grows into.
    #[inline(always)]
    pub fn allocation_alignment(&self) -> usize {
        self.inner.allocation_alignment(A::ALIGN)
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

impl<A> OwnedAlignedBuffer<A>
where
    A: AtLeast<Align16>,
{
    /// Contents as `u128`s.
    ///
    /// Only whole `u128`s are included: when the length is not a multiple of `u128` size, the
    /// bytes after the last whole `u128` are not.
    #[inline(always)]
    pub const fn as_u128_slice(&self) -> &[u128] {
        let len = self.len() as usize / size_of::<u128>();
        // SAFETY: Not null, correctly aligned (contents are aligned to at least `u128`), and the
        // first `len` elements are initialized, since they are within `self.len()` bytes, which
        // are initialized (a protected invariant of the implementation)
        unsafe { slice::from_raw_parts(self.as_ptr(), len) }
    }

    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Contents are aligned to at least `u128`"
    )]
    pub const fn as_ptr(&self) -> *const u128 {
        self.as_byte_ptr().cast::<u128>()
    }

    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Contents are aligned to at least `u128`"
    )]
    pub const fn as_mut_ptr(&mut self) -> *mut u128 {
        self.as_mut_byte_ptr().cast::<u128>()
    }
}

/// Shared aligned buffer for executor purposes.
///
/// See [`OwnedAlignedBuffer`] for a version that can be mutated.
///
/// Contents are aligned to `A`, by default 16 bytes (128 bits), which is the largest alignment
/// required by primitive types and by extension any type that implements `TrivialType`/`IoType`.
#[derive(Debug)]
#[repr(transparent)]
pub struct SharedAlignedBuffer<A = Align16>
where
    A: Alignment,
{
    /// Empty buffer doesn't hold a reference to an allocation and points to the empty allocation
    /// instead, all other buffers hold one reference
    inner: InnerBuffer,
    alignment: PhantomData<A>,
}

impl<A> Default for SharedAlignedBuffer<A>
where
    A: Alignment,
{
    #[inline(always)]
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<A> Clone for SharedAlignedBuffer<A>
where
    A: Alignment,
{
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

        Self::from_inner(InnerBuffer {
            buffer: self.inner.buffer,
            len: self.inner.len,
        })
    }
}

impl<A> Drop for SharedAlignedBuffer<A>
where
    A: Alignment,
{
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

impl<A> Deref for SharedAlignedBuffer<A>
where
    A: Alignment,
{
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

// SAFETY: Heap-allocated data structure, points to the same memory if moved
unsafe impl<A> StableDeref for SharedAlignedBuffer<A> where A: Alignment {}
// SAFETY: Inner buffer is exactly the same and points to the same memory after clone
unsafe impl<A> CloneStableDeref for SharedAlignedBuffer<A> where A: Alignment {}
// SAFETY: Inner buffer is exactly the same and points to the same memory after clone
unsafe impl<A> CloneableCart for SharedAlignedBuffer<A> where A: Alignment {}

impl<A> SharedAlignedBuffer<A>
where
    A: Alignment,
{
    /// Create a new instance from provided bytes.
    ///
    /// Allocation is aligned to the alignment of the contents, but at least 16 bytes.
    ///
    /// # Panics
    /// If `bytes.len()` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        OwnedAlignedBuffer::from_bytes(bytes).into_shared()
    }

    /// Empty buffer that is not backed by an allocation
    pub const EMPTY: Self = Self::from_inner(InnerBuffer::EMPTY);

    #[inline(always)]
    const fn from_inner(inner: InnerBuffer) -> Self {
        Self {
            inner,
            alignment: PhantomData,
        }
    }

    #[inline(always)]
    fn into_inner(self) -> InnerBuffer {
        let this = ManuallyDrop::new(self);
        // SAFETY: Moved out of `ManuallyDrop` that is never used again
        unsafe { (&raw const this.inner).read() }
    }

    /// Static reference to an empty buffer
    #[inline(always)]
    pub const fn empty_ref() -> &'static Self {
        // SAFETY: `repr(transparent)` wrapper around `InnerBuffer`, and empty buffer is aligned to
        // any supported alignment
        unsafe { &*(&raw const EMPTY_INNER_BUFFER).cast::<Self>() }
    }

    /// Convert into owned buffer.
    ///
    /// If this is the last shared instance, then allocation will be reused, otherwise a new
    /// allocation will be created.
    #[inline(always)]
    pub fn into_owned(self) -> OwnedAlignedBuffer<A> {
        if self.is_empty() {
            return OwnedAlignedBuffer::EMPTY;
        }

        // Synchronizes with decrements in other instances, such that their accesses to the
        // contents happen before modifications through the owned buffer
        if self.inner.strong_count_ref().load(Ordering::Acquire) == 1 {
            // This was the last reference, so owned buffer is not shared
            OwnedAlignedBuffer::from_inner(self.into_inner())
        } else {
            OwnedAlignedBuffer::from_inner(self.inner.copy(A::ALIGN))
        }
    }

    /// Convert into a buffer with a weaker alignment of the contents, which is free
    #[inline(always)]
    #[must_use = "The buffer is dropped otherwise"]
    pub fn into_start_align<B>(self) -> SharedAlignedBuffer<B>
    where
        A: AtLeast<B>,
        B: Alignment,
    {
        SharedAlignedBuffer::from_inner(self.into_inner())
    }

    /// Convert into a buffer with a different alignment of the contents, which is free.
    ///
    /// Returns the buffer back if its contents are not aligned to `B`.
    #[inline(always)]
    pub fn try_into_start_align<B>(self) -> Result<SharedAlignedBuffer<B>, Self>
    where
        B: Alignment,
    {
        if self.inner.is_aligned_to(B::ALIGN) {
            Ok(SharedAlignedBuffer::from_inner(self.into_inner()))
        } else {
            Err(self)
        }
    }

    #[inline(always)]
    pub const fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    /// Pointer to the first byte of the contents
    #[inline(always)]
    pub const fn as_byte_ptr(&self) -> *const u8 {
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

    /// Alignment of the allocation in bytes.
    ///
    /// It is not smaller than the alignment of the contents, unless the contents happen to be
    /// aligned more strictly than the allocation (see [`Self::try_into_start_align()`]). Empty
    /// buffer without an allocation reports the alignment it grows into.
    #[inline(always)]
    pub fn allocation_alignment(&self) -> usize {
        self.inner.allocation_alignment(A::ALIGN)
    }
}

impl<A> SharedAlignedBuffer<A>
where
    A: AtLeast<Align16>,
{
    #[inline(always)]
    #[expect(
        clippy::cast_ptr_alignment,
        reason = "Contents are aligned to at least `u128`"
    )]
    pub const fn as_ptr(&self) -> *const u128 {
        self.as_byte_ptr().cast::<u128>()
    }
}
