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

use crate::alignment::{Align1, Align16, Align4096, Alignment, AtLeast};
use alloc::alloc::{alloc, alloc_zeroed, dealloc, handle_alloc_error, realloc};
use core::alloc::Layout;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::{Bound, Deref, DerefMut, RangeBounds};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};
use core::{fmt, hint, ptr, slice};
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
    /// The first byte of the contents
    buffer: NonNull<u8>,
    /// Offset of the contents from the beginning of the data region of the allocation, which has
    /// the header right before it
    offset: u32,
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
        offset: 0,
        len: 0,
    };

    /// Layout of an allocation for a specified capacity and alignment.
    ///
    /// The header is in front of the data region, and the size of the allocation is a multiple of
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
    /// The buffer must be backed by an allocation (have non-zero total capacity).
    #[inline(always)]
    unsafe fn allocated_layout(&self) -> Layout {
        let alignment = self.alignment();
        let size = alignment + (self.total_capacity() as usize).next_multiple_of(alignment);
        // SAFETY: Allocated with the same layout before, which was checked by `Self::layout()`
        unsafe { Layout::from_size_align_unchecked(size, alignment) }
    }

    /// Allocates a new buffer with a header in front of the data region, contents start at
    /// `offset` within the data region.
    ///
    /// `alignment` must be a supported alignment that is not smaller than
    /// `MIN_ALLOCATION_ALIGNMENT`, `offset` must not be larger than `total_capacity`. Zero total
    /// capacity results in an empty buffer that is not backed by an allocation.
    ///
    /// # Panics
    /// If `total_capacity` is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or allocation fails
    #[inline(always)]
    fn allocate(total_capacity: u32, alignment: usize, offset: u32) -> Self {
        // SAFETY: Global allocator's function
        unsafe { Self::allocate_with(total_capacity, alignment, offset, alloc) }
    }

    /// Allocates a new buffer with zeroed data region, see [`Self::allocate()`] for details
    #[inline(always)]
    fn allocate_zeroed(total_capacity: u32, alignment: usize, offset: u32) -> Self {
        // SAFETY: Global allocator's function
        unsafe { Self::allocate_with(total_capacity, alignment, offset, alloc_zeroed) }
    }

    /// # Safety
    /// `allocator` must allocate memory with global allocator
    #[inline(always)]
    unsafe fn allocate_with(
        total_capacity: u32,
        alignment: usize,
        offset: u32,
        allocator: unsafe fn(Layout) -> *mut u8,
    ) -> Self {
        debug_assert!(
            alignment.is_power_of_two()
                && (MIN_ALLOCATION_ALIGNMENT..=MAX_ALIGNMENT).contains(&alignment),
            "Unsupported alignment {alignment}"
        );
        debug_assert!(
            offset <= total_capacity,
            "Offset {offset} is larger than capacity {total_capacity}"
        );

        if total_capacity == 0 {
            return Self::EMPTY;
        }

        let Some(layout) = Self::layout(total_capacity, alignment) else {
            panic!("Allocation of capacity {total_capacity} has failed: too large");
        };
        // SAFETY: Non-zero size, allocates with global allocator, guaranteed by the caller
        let allocation = unsafe { allocator(layout) };
        let Some(allocation) = NonNull::new(allocation) else {
            handle_alloc_error(layout);
        };
        // SAFETY: The data region starts right after the space reserved for the header at the
        // beginning of the allocation, which is as large as the alignment
        let data = unsafe { allocation.byte_add(alignment) };

        // SAFETY: The header is right before the data region and is correctly aligned, since the
        // data region is aligned to at least the size of the header
        unsafe {
            data.cast::<Header>().sub(1).write(Header {
                strong_count: AtomicUsize::new(1),
                capacity: total_capacity,
                alignment_log2: alignment.ilog2(),
            });
        }

        Self {
            // SAFETY: Offset is within the data region
            buffer: unsafe { data.byte_add(offset as usize) },
            offset,
            len: 0,
        }
    }

    /// Changes capacity of the buffer while preserving its contents and their offset, the buffer
    /// must not be shared.
    ///
    /// The new allocation is aligned to at least `min_alignment`, which must be a supported
    /// alignment.
    ///
    /// # Panics
    /// If `capacity` with offset is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`] or allocation
    /// fails
    #[inline(always)]
    fn resize(&mut self, capacity: u32, min_alignment: usize) {
        let old_total_capacity = self.total_capacity();
        if old_total_capacity == 0 {
            // Empty buffer is not backed by an allocation, and it has no contents to preserve
            *self = Self::allocate(capacity, min_alignment.max(MIN_ALLOCATION_ALIGNMENT), 0);
            return;
        }

        let Some(total_capacity) = self.offset.checked_add(capacity) else {
            panic!(
                "Resize from {} to {capacity} has failed: too large",
                self.capacity()
            );
        };

        let alignment = self.alignment();
        // `realloc()` is only used for allocations aligned to 16 bytes when that is enough for the
        // contents: it can't change the alignment, and standard library's `realloc()` copies the
        // whole allocation for larger alignments anyway. Contents with an offset are not
        // reallocated either, since bytes before them would be copied too
        let reallocate_in_place = alignment == MIN_ALLOCATION_ALIGNMENT
            && min_alignment <= MIN_ALLOCATION_ALIGNMENT
            && self.offset == 0;
        if !reallocate_in_place {
            // Copy just the contents into a new allocation. Contents with an offset in an
            // allocation with smaller alignment could only be aligned to `min_alignment` by chance,
            // so the offset is reduced to keep the alignment then.
            let misalignment = u32::try_from(self.offset as usize % min_alignment)
                .expect("Smaller than alignment, which is at most 4096; qed");
            let offset = self.offset - misalignment;
            let new = Self::allocate(
                total_capacity - (self.offset - offset),
                alignment.max(min_alignment),
                offset,
            );
            // SAFETY: Not shared, new buffer has enough capacity for the contents
            unsafe {
                self.move_into(new);
            }
            return;
        }

        let Some(new_layout) = Self::layout(total_capacity, alignment) else {
            panic!("Realloc from {old_total_capacity} to {total_capacity} has failed: too large");
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
        // SAFETY: The data region starts right after the space reserved for the header at the
        // beginning of the allocation, which is as large as the alignment, and offset is zero
        self.buffer = unsafe { allocation.byte_add(alignment) };
        // SAFETY: Reallocation preserved the header, and the buffer is not shared
        unsafe {
            (*self.header_ptr().as_ptr()).capacity = total_capacity;
        }
    }

    /// Moves contents into a new buffer and deallocates the current one.
    ///
    /// # Safety
    /// The buffer must not be shared, the new buffer must be a different allocation with enough
    /// capacity for the contents.
    #[inline(always)]
    unsafe fn move_into(&mut self, mut new: Self) {
        debug_assert!(new.capacity() >= self.len, "Not enough capacity");

        // SAFETY: Different allocations, `len` bytes are initialized and fit into the new
        // capacity, guaranteed by the caller
        unsafe {
            new.buffer
                .copy_from_nonoverlapping(self.buffer, self.len as usize);
        }
        new.len = self.len;
        if self.total_capacity() != 0 {
            // SAFETY: Backed by an allocation and not shared, guaranteed by the caller
            unsafe {
                self.deallocate();
            }
        }
        *self = new;
    }

    /// Moves contents to a different offset within the same allocation.
    ///
    /// # Safety
    /// The buffer must not be shared, the contents must fit into the data region at a new offset.
    #[inline(always)]
    unsafe fn move_to_offset(&mut self, offset: u32) {
        debug_assert!(
            u64::from(offset) + u64::from(self.len) <= u64::from(self.total_capacity()),
            "Contents don't fit"
        );

        // SAFETY: Within the data region, guaranteed by the caller
        let buffer = unsafe { self.data_ptr().byte_add(offset as usize) };
        // SAFETY: Both ranges are within the same allocation, `copy_from` handles overlapping
        // ranges, `len` bytes are initialized
        unsafe {
            buffer.copy_from(self.buffer, self.len as usize);
        }
        self.buffer = buffer;
        self.offset = offset;
    }

    /// Deallocates the buffer.
    ///
    /// # Safety
    /// The buffer must be backed by an allocation (have non-zero total capacity) and not be
    /// referenced anymore.
    #[inline(always)]
    unsafe fn deallocate(&self) {
        // SAFETY: Allocated with global allocator and this layout, guaranteed by the caller to be
        // backed by an allocation that is not referenced anymore
        unsafe {
            dealloc(self.allocation_ptr().as_ptr(), self.allocated_layout());
        }
    }

    /// Creates an owned copy of the contents without offset, aligned to `alignment`, which must be
    /// a supported alignment
    #[inline(always)]
    fn copy(&self, alignment: usize) -> Self {
        let mut new = Self::allocate(self.len, alignment.max(MIN_ALLOCATION_ALIGNMENT), 0);
        // SAFETY: Different allocations, `len` bytes are initialized and fit into the new capacity
        unsafe {
            new.buffer
                .copy_from_nonoverlapping(self.buffer, self.len as usize);
        }
        new.len = self.len;
        new
    }

    /// Creates a new reference to a part of the contents of a shared buffer, which must be
    /// non-empty and within the contents
    #[inline(always)]
    fn slice(&self, start: u32, len: u32) -> Self {
        debug_assert!(
            len != 0 && u64::from(start) + u64::from(len) <= u64::from(self.len),
            "Slice out of bounds"
        );
        self.increment_strong_count();

        Self {
            // SAFETY: Within the contents
            buffer: unsafe { self.buffer.byte_add(start as usize) },
            offset: self.offset + start,
            len,
        }
    }

    /// Increments reference counter of a shared buffer, which must not be empty
    #[inline(always)]
    fn increment_strong_count(&self) {
        let strong_count = self.strong_count_ref();
        let old_count = strong_count.fetch_add(1, Ordering::Relaxed);
        // Leaked clones could wrap a counter narrower than 64 bits around, and a drop would then
        // free the buffer while other clones still use it
        if usize::BITS < u64::BITS && old_count > isize::MAX.cast_unsigned() {
            strong_count.fetch_sub(1, Ordering::Relaxed);
            panic!("Too many clones of a shared aligned buffer");
        }
    }

    #[inline(always)]
    const fn data_ptr(&self) -> NonNull<u8> {
        // SAFETY: Contents are at offset within the data region
        unsafe { self.buffer.byte_sub(self.offset as usize) }
    }

    #[inline(always)]
    const fn header_ptr(&self) -> NonNull<Header> {
        // SAFETY: The header is right before the data region
        unsafe { self.data_ptr().cast::<Header>().sub(1) }
    }

    #[inline(always)]
    const fn header(&self) -> &Header {
        // SAFETY: The header is initialized in the constructor and lives as long as the buffer
        unsafe { self.header_ptr().as_ref() }
    }

    #[inline(always)]
    fn allocation_ptr(&self) -> NonNull<u8> {
        // SAFETY: The data region starts right after the space reserved for the header at the
        // beginning of the allocation, which is as large as the alignment
        unsafe { self.data_ptr().byte_sub(self.alignment()) }
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
    const fn offset(&self) -> u32 {
        self.offset
    }

    /// Capacity from the beginning of the contents
    #[inline(always)]
    const fn capacity(&self) -> u32 {
        self.total_capacity() - self.offset
    }

    /// Capacity of the whole data region
    #[inline(always)]
    const fn total_capacity(&self) -> u32 {
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
        if self.inner.total_capacity() != 0 {
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

impl<A> sealed::Sealed for OwnedAlignedBuffer<A> where A: Alignment {}

impl<A> AlignedBuffer for OwnedAlignedBuffer<A>
where
    A: Alignment,
{
    type Alignment = A;

    #[inline(always)]
    fn len(&self) -> u32 {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline(always)]
    fn offset(&self) -> u32 {
        self.offset()
    }

    #[inline(always)]
    fn allocation_alignment(&self) -> usize {
        self.allocation_alignment()
    }

    #[inline(always)]
    fn into_shared(self) -> SharedAlignedBuffer<Self::Alignment> {
        self.into_shared()
    }
}

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
    /// Allocates as much as the length of the buffer with the same alignment, without offset
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::from_inner(self.inner.copy(self.inner.alignment().max(A::ALIGN)))
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
            0,
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

    /// Create a new instance of specified length filled with zeroes.
    ///
    /// Allocation is aligned to the alignment of the contents, but at least 16 bytes. Zeroed memory
    /// is allocated directly, which is cheaper than writing zeroes into it, especially for large
    /// buffers.
    ///
    /// # Panics
    /// If `len` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn zeroed(len: u32) -> Self {
        Self::zeroed_with_alignment(len, A::ALIGN.max(MIN_ALLOCATION_ALIGNMENT))
    }

    /// Create a new instance of specified length filled with zeroes and allocation aligned to
    /// `AA`, see [`Self::zeroed()`] for details.
    ///
    /// # Panics
    /// If `len` is larger than [`Self::MAX_CAPACITY`]
    #[inline(always)]
    pub fn zeroed_aligned<AA>(len: u32) -> Self
    where
        AA: AtLeast<A> + AtLeast<Align16>,
    {
        Self::zeroed_with_alignment(len, AA::ALIGN)
    }

    #[inline(always)]
    fn zeroed_with_alignment(len: u32, alignment: usize) -> Self {
        let mut instance = Self::from_inner(InnerBuffer::allocate_zeroed(len, alignment, 0));
        // SAFETY: The whole data region is zeroed, which is a valid initialized value
        unsafe {
            instance.set_len(len);
        }
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
        Self::from_inner(InnerBuffer::allocate(capacity, AA::ALIGN, 0))
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

    /// Spare capacity of the buffer after its contents, which can be written to and then included
    /// in the contents with [`Self::set_len()`]
    #[inline(always)]
    pub const fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        let len = self.len() as usize;
        let spare_capacity = (self.capacity() - self.len()) as usize;
        // SAFETY: Spare capacity is within the allocation right after the contents, the buffer is
        // not shared, and bytes are treated as potentially uninitialized
        unsafe {
            slice::from_raw_parts_mut(
                self.as_mut_byte_ptr().add(len).cast::<MaybeUninit<u8>>(),
                spare_capacity,
            )
        }
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
    /// If `capacity` with [`Self::offset()`] is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn ensure_capacity(&mut self, capacity: u32) {
        if capacity > self.capacity() {
            self.inner.resize(capacity, A::ALIGN);
        }
    }

    /// Will re-allocate if capacity is not enough to store provided bytes.
    ///
    /// # Panics
    /// If `bytes.len()` with [`Self::offset()`] is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline(always)]
    pub fn copy_from_slice(&mut self, bytes: &[u8]) {
        let Ok(len) = u32::try_from(bytes.len()) else {
            panic!("Too many bytes {}", bytes.len());
        };

        if len > self.capacity() {
            // Doubling the capacity is capped by the largest one `Layout` allows
            self.inner.resize(
                len.max(
                    self.capacity()
                        .saturating_mul(2)
                        .min(MAX_CAPACITY - self.offset()),
                ),
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
    /// Returns `false` and leaves the buffer unchanged if `self.len() + bytes.len()` with
    /// [`Self::offset()`] is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`].
    ///
    /// # Panics
    /// If memory re-allocation fails
    #[inline(always)]
    #[must_use = "Check for overflow"]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let Ok(len) = u32::try_from(bytes.len()) else {
            return false;
        };

        let Some(new_len) = self.len().checked_add(len) else {
            return false;
        };
        // Offset is not larger than the capacity of the allocation, so this doesn't underflow
        if new_len > MAX_CAPACITY - self.offset() {
            return false;
        }

        if new_len > self.capacity() {
            // Doubling the capacity is capped by the largest one `Layout` allows
            self.inner.resize(
                new_len.max(
                    self.capacity()
                        .saturating_mul(2)
                        .min(MAX_CAPACITY - self.offset()),
                ),
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

    /// Capacity from the beginning of the contents
    #[inline(always)]
    pub const fn capacity(&self) -> u32 {
        self.inner.capacity()
    }

    /// Offset of the contents from the beginning of the data region of the allocation
    #[inline(always)]
    pub const fn offset(&self) -> u32 {
        self.inner.offset()
    }

    /// Alignment of the allocation in bytes.
    ///
    /// It is not smaller than the alignment of the contents, unless the contents happen to be
    /// aligned more strictly than the allocation (see [`Self::try_into_start_align()`] and
    /// [`Self::realign()`]). Empty buffer without an allocation reports the alignment it grows
    /// into.
    #[inline(always)]
    pub fn allocation_alignment(&self) -> usize {
        self.inner.allocation_alignment(A::ALIGN)
    }

    /// Shorten the buffer, keeping the first `len` bytes.
    ///
    /// Does nothing if `len` is not smaller than the current length.
    #[inline(always)]
    pub fn truncate(&mut self, len: u32) {
        if len < self.len() {
            self.inner.len = len;
        }
    }

    /// Move the contents to `offset` from the beginning of the data region, with at least
    /// `capacity` bytes available from there, in an allocation aligned to at least `AA`.
    ///
    /// Contents are moved within the current allocation when it is large enough and sufficiently
    /// aligned, otherwise they are copied into a new allocation once.
    ///
    /// Returns `false` and leaves the buffer unchanged if `offset` is not a multiple of the
    /// alignment of the contents or `offset` with `capacity` is larger than
    /// [`OwnedAlignedBuffer::MAX_CAPACITY`].
    #[inline]
    #[must_use = "Check for invalid offset or capacity"]
    #[cfg_attr(
        target_pointer_width = "64",
        expect(
            clippy::absurd_extreme_comparisons,
            reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
        )
    )]
    pub fn relocate<AA>(&mut self, offset: u32, capacity: u32) -> bool
    where
        AA: AtLeast<A> + AtLeast<Align16>,
    {
        if !(offset as usize).is_multiple_of(A::ALIGN) {
            return false;
        }
        let capacity = capacity.max(self.len());
        let Some(total_capacity) = offset.checked_add(capacity) else {
            return false;
        };
        if total_capacity > MAX_CAPACITY {
            return false;
        }

        if self.inner.total_capacity() >= total_capacity && self.inner.alignment() >= AA::ALIGN {
            // SAFETY: Not shared, contents fit into the data region at a new offset
            unsafe {
                self.inner.move_to_offset(offset);
            }
        } else {
            // SAFETY: Not shared, a new allocation with enough capacity for the contents
            unsafe {
                self.inner
                    .move_into(InnerBuffer::allocate(total_capacity, AA::ALIGN, offset));
            }
        }

        true
    }

    /// Convert into a buffer with a different alignment of the contents.
    ///
    /// Free if the contents are already aligned to `B`. Otherwise, the contents are moved within
    /// the current allocation to the closest offset aligned to `B` if possible, even if the
    /// allocation itself is aligned less strictly, or copied into a new allocation aligned to `B`
    /// once.
    #[inline]
    #[must_use = "The buffer is dropped otherwise"]
    pub fn realign<B>(mut self) -> OwnedAlignedBuffer<B>
    where
        B: Alignment,
    {
        let misalignment = u32::try_from(self.inner.buffer.addr().get() % B::ALIGN)
            .expect("Smaller than alignment, which is at most 4096; qed");
        if misalignment != 0 {
            let alignment = u32::try_from(B::ALIGN).expect("At most 4096; qed");
            let len = self.len();
            let total_capacity = self.inner.total_capacity();
            // Moving contents backwards always fits, moving them forward only fits with enough
            // capacity
            let maybe_offset = self.offset().checked_sub(misalignment).or_else(|| {
                self.offset()
                    .checked_add(alignment - misalignment)
                    .filter(|&offset| {
                        u64::from(offset) + u64::from(len) <= u64::from(total_capacity)
                    })
            });

            if let Some(offset) = maybe_offset {
                // SAFETY: Not shared, contents fit into the data region at a new offset
                unsafe {
                    self.inner.move_to_offset(offset);
                }
            } else {
                let new = InnerBuffer::allocate(
                    self.capacity(),
                    B::ALIGN.max(MIN_ALLOCATION_ALIGNMENT),
                    0,
                );
                // SAFETY: Not shared, a new allocation with enough capacity for the contents
                unsafe {
                    self.inner.move_into(new);
                }
            }
        }

        OwnedAlignedBuffer::from_inner(self.into_inner())
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
            self.inner.increment_strong_count();
        }

        Self::from_inner(InnerBuffer {
            buffer: self.inner.buffer,
            offset: self.inner.offset,
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

impl<A> sealed::Sealed for SharedAlignedBuffer<A> where A: Alignment {}

impl<A> AlignedBuffer for SharedAlignedBuffer<A>
where
    A: Alignment,
{
    type Alignment = A;

    #[inline(always)]
    fn len(&self) -> u32 {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    #[inline(always)]
    fn offset(&self) -> u32 {
        self.offset()
    }

    #[inline(always)]
    fn allocation_alignment(&self) -> usize {
        self.allocation_alignment()
    }

    #[inline(always)]
    fn into_shared(self) -> SharedAlignedBuffer<Self::Alignment> {
        self
    }
}

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
    /// If this is the last reference to the allocation, then allocation will be reused with the
    /// same offset of the contents, otherwise contents will be copied into a new allocation with
    /// the same alignment.
    #[inline(always)]
    pub fn into_owned(self) -> OwnedAlignedBuffer<A> {
        match self.try_into_owned() {
            Ok(owned) => owned,
            Err(shared) => OwnedAlignedBuffer::from_inner(
                shared.inner.copy(shared.inner.alignment().max(A::ALIGN)),
            ),
        }
    }

    /// Convert into owned buffer if this is the last reference to the allocation, which is reused
    /// with the same offset of the contents.
    ///
    /// Returns the buffer back if the allocation is still shared.
    #[inline(always)]
    pub fn try_into_owned(self) -> Result<OwnedAlignedBuffer<A>, Self> {
        if self.is_empty() {
            return Ok(OwnedAlignedBuffer::EMPTY);
        }

        // Synchronizes with decrements in other instances, such that their accesses to the
        // contents happen before modifications through the owned buffer
        if self.inner.strong_count_ref().load(Ordering::Acquire) == 1 {
            // This was the last reference, so owned buffer is not shared
            Ok(OwnedAlignedBuffer::from_inner(self.into_inner()))
        } else {
            Err(self)
        }
    }

    /// Make sure the buffer doesn't keep a larger allocation alive than its contents need.
    ///
    /// Contents are copied into a new allocation without offset, unless they already cover the
    /// whole data region of the allocation. This is useful for buffers that are kept for a long
    /// time and may be a part of a much larger allocation, see [`Self::get_slice()`].
    ///
    /// The new allocation is aligned to the alignment of the contents, but at least 16 bytes,
    /// rather than the alignment of the original allocation, which may be much larger than
    /// necessary.
    #[inline]
    #[must_use = "The buffer is dropped otherwise"]
    pub fn into_standalone(self) -> Self {
        if self.offset() == 0 && self.len() == self.inner.total_capacity() {
            self
        } else {
            Self::from_inner(self.inner.copy(A::ALIGN))
        }
    }

    /// Create a new buffer that references a part of the contents of this one without copying
    /// them.
    ///
    /// Returns `None` if `range` is out of bounds. Alignment of the contents is not guaranteed
    /// anymore, use [`SharedAlignedBuffer::try_into_start_align()`] if necessary. The new buffer
    /// keeps the whole allocation alive, see [`Self::into_standalone()`].
    #[inline]
    #[must_use = "The slice is dropped otherwise"]
    pub fn get_slice<R>(&self, range: R) -> Option<SharedAlignedBuffer<Align1>>
    where
        R: RangeBounds<u32>,
    {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start.checked_add(1)?,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end.checked_add(1)?,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        if start > end || end > self.len() {
            return None;
        }
        if start == end {
            return Some(SharedAlignedBuffer::EMPTY);
        }

        Some(SharedAlignedBuffer::from_inner(
            self.inner.slice(start, end - start),
        ))
    }

    /// Shorten the buffer, keeping the first `len` bytes.
    ///
    /// Does nothing if `len` is not smaller than the current length. Empty buffer doesn't keep the
    /// allocation alive.
    #[inline(always)]
    pub fn truncate(&mut self, len: u32) {
        if len == 0 {
            *self = Self::EMPTY;
        } else if len < self.len() {
            self.inner.len = len;
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

    /// Capacity from the beginning of the contents
    #[inline(always)]
    pub const fn capacity(&self) -> u32 {
        self.inner.capacity()
    }

    /// Offset of the contents from the beginning of the data region of the allocation
    #[inline(always)]
    pub const fn offset(&self) -> u32 {
        self.inner.offset()
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

/// Common interface of [`OwnedAlignedBuffer`] and [`SharedAlignedBuffer`].
///
/// Contents don't move when the buffer is moved, so a buffer can be moved into an operation that
/// accesses its memory, like asynchronous I/O, which then returns it back.
pub trait AlignedBuffer:
    sealed::Sealed + Deref<Target = [u8]> + StableDeref + fmt::Debug + Send + Sync + 'static
{
    /// Alignment of the contents
    type Alignment: Alignment;

    /// Length of the contents
    fn len(&self) -> u32;

    /// Whether the contents are empty
    fn is_empty(&self) -> bool;

    /// Offset of the contents from the beginning of the data region of the allocation
    fn offset(&self) -> u32;

    /// Alignment of the allocation in bytes, see [`OwnedAlignedBuffer::allocation_alignment()`]
    fn allocation_alignment(&self) -> usize;

    /// Convert into shared buffer, which reuses the allocation
    fn into_shared(self) -> SharedAlignedBuffer<Self::Alignment>;
}
