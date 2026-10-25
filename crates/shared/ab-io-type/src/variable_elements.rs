#[cfg(test)]
mod tests;

use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::{NON_ZERO_SIZED, TrivialType, try_skip_valid};
use crate::{DerefWrapper, IoType, IoTypeOptional};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{ptr, slice};

/// Whether the first `size` bytes behind `ptr` are valid elements of [`VariableElements`].
///
/// # Safety
/// `ptr` is valid for reads of `size` bytes, which are initialized and not written to during the
/// call.
#[inline(always)]
unsafe fn contents_are_valid<Element>(ptr: NonNull<Element>, size: u32) -> bool
where
    Element: TrivialType,
    [(); NON_ZERO_SIZED::<Element>]:,
{
    // TODO: Technically redundant with the where-clause, which is not enforced in generic code
    //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
    if size_of::<Element>() == 0 {
        return false;
    }
    // SAFETY: `ptr` is valid for reads of `size` initialized bytes, which are not written to
    // during the call (function contract)
    let bytes = unsafe { slice::from_raw_parts(ptr.as_ptr().cast::<u8>(), size as usize) };
    try_skip_valid::<Element>(bytes, bytes.len() / size_of::<Element>()).is_some()
}

/// Container for storing variable number of elements.
///
/// It is a view of memory that is borrowed for `'a`.
///
/// `RECOMMENDED_ALLOCATION` is what is being used when a host needs to allocate memory for call
/// into guest, but guest may receive an allocation with more or less memory in practice depending
/// on other circumstances, like when called from another contract with specific allocation
/// specified.
#[derive(Debug)]
#[repr(C)]
pub struct VariableElements<'a, Element, const RECOMMENDED_ALLOCATION: u32 = 0>
where
    Element: TrivialType,
{
    elements: NonNull<Element>,
    size: NonNull<u32>,
    capacity: u32,
    memory: PhantomData<&'a ()>,
    // TODO: Technically redundant with the where-clause, which is not enforced in generic code
    //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed. Every instance
    //  evaluates the check through this field. It is initialized in `const` blocks, which are
    //  evaluated even if optimizations remove the field.
    _supported: [(); NON_ZERO_SIZED::<Element>],
}

// SAFETY: Low-level (effectively internal) implementation that upholds safety requirements
unsafe impl<Element, const RECOMMENDED_ALLOCATION: u32> IoType
    for VariableElements<'_, Element, RECOMMENDED_ALLOCATION>
where
    Element: TrivialType,
    [(); NON_ZERO_SIZED::<Element>]:,
{
    const METADATA: &[u8] = {
        const fn metadata(
            recommended_allocation: u32,
            inner_metadata: &[u8],
        ) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[
                &[IoTypeMetadataKind::VariableElements as u8],
                &recommended_allocation.to_le_bytes(),
                inner_metadata,
            ])
        }

        // The reference extends the lifetime of metadata scratch to that of the constant
        let (metadata_scratch, metadata_size) =
            &metadata(RECOMMENDED_ALLOCATION, Element::METADATA);
        metadata_scratch.split_at(*metadata_size).0
    };

    // TODO: Use `[Element; U32_TO_USIZE::<RECOMMENDED_ALLOCATION>]` with `generic_const_args`
    type PointerType = Element;

    type WithLifetime<'b> = VariableElements<'b, Element, RECOMMENDED_ALLOCATION>;

    type Reborrowed<'b>
        = VariableElements<'b, Element, RECOMMENDED_ALLOCATION>
    where
        Self: 'b;

    #[inline(always)]
    fn size(&self) -> u32 {
        self.size()
    }

    #[inline(always)]
    fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size <= self.capacity,
            "`set_size` called with invalid input {size} for capacity {}",
            self.capacity
        );
        debug_assert!(
            size.is_multiple_of(Element::SIZE),
            "`set_size` called with invalid input {size} for element size {}",
            Element::SIZE
        );

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self::WithLifetime<'a>> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        debug_assert!(
            size.is_multiple_of(Element::SIZE),
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );

        DerefWrapper(VariableElements {
            elements: *ptr,
            size: NonNull::from_ref(size),
            capacity,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        })
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self::WithLifetime<'a>> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        debug_assert!(
            size.is_multiple_of(Element::SIZE),
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );

        DerefWrapper(VariableElements {
            elements: *ptr,
            size: NonNull::from_mut(size),
            capacity,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        })
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn try_from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> Option<impl Deref<Target = Self::WithLifetime<'a>> + 'a> {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        debug_assert!(
            size.is_multiple_of(Element::SIZE),
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );
        // SAFETY: `ptr` is valid for reads of `size` initialized bytes, which are not written to
        // during the call (function contract)
        if !unsafe { contents_are_valid(*ptr, *size) } {
            return None;
        }

        // SAFETY: `size` and `capacity` are valid, `ptr` is aligned and the memory is not written
        // to while the returned value exists (function contract), and the first `size` bytes are
        // valid elements as checked above
        Some(unsafe { Self::from_ptr(ptr, size, capacity) })
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn try_from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> Option<impl DerefMut<Target = Self::WithLifetime<'a>> + 'a> {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        debug_assert!(
            size.is_multiple_of(Element::SIZE),
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );
        // SAFETY: `ptr` is valid for reads of the first `size` bytes, which are initialized, and
        // the memory is not accessed otherwise during the call (function contract)
        if !unsafe { contents_are_valid(*ptr, *size) } {
            return None;
        }

        // SAFETY: `size` and `capacity` are valid, `ptr` is aligned, valid for writes of `capacity`
        // bytes, the memory and `size` are not accessed otherwise for as long as
        // `Self::from_mut_ptr()` requires (function contract), and the first `size` bytes are
        // valid elements as checked above
        Some(unsafe { Self::from_mut_ptr(ptr, size, capacity) })
    }

    #[inline(always)]
    fn as_ptr(&self) -> NonNull<Self::PointerType> {
        self.elements
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> NonNull<Self::PointerType> {
        self.elements
    }

    #[inline(always)]
    fn reborrow_mut(&mut self) -> Self::Reborrowed<'_> {
        VariableElements {
            elements: self.elements,
            size: self.size,
            capacity: self.capacity,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        }
    }
}

// SAFETY: Size `0` means there are no initialized elements, contents are only accessed up to the
// size
unsafe impl<Element, const RECOMMENDED_ALLOCATION: u32> IoTypeOptional
    for VariableElements<'_, Element, RECOMMENDED_ALLOCATION>
where
    Element: TrivialType,
    [(); NON_ZERO_SIZED::<Element>]:,
{
}

// Sizes are converted to the number of elements by dividing by the element size, so elements must
// not be zero-sized, which the `IoType` implementation checks
impl<'a, Element, const RECOMMENDED_ALLOCATION: u32>
    VariableElements<'a, Element, RECOMMENDED_ALLOCATION>
where
    Element: TrivialType,
    Self: IoType,
{
    /// Create a new shared instance from provided memory buffer.
    ///
    /// NOTE: size is specified in bytes, not elements.
    ///
    /// Returns `None` if `size_of_val(buffer) != size`.
    //
    // `impl Deref` is returned instead of `Self`, since methods that take `&mut self` would
    // otherwise write into shared memory.
    #[inline(always)]
    pub const fn from_buffer(
        buffer: &'a [Element],
        size: &'a u32,
    ) -> Option<impl Deref<Target = Self> + 'a> {
        if size_of_val(buffer) != *size as usize {
            return None;
        }

        Some(DerefWrapper(Self {
            elements: NonNull::new(buffer.as_ptr().cast_mut()).expect("Not null; qed"),
            size: NonNull::from_ref(size),
            capacity: *size,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        }))
    }

    /// Create a new exclusive instance from provided memory buffer.
    ///
    /// NOTE: size is specified in bytes, not elements.
    ///
    /// Returns `None` if `size_of_val(buffer) != size`.
    #[inline(always)]
    pub fn from_buffer_mut(buffer: &'a mut [Element], size: &'a mut u32) -> Option<Self> {
        if size_of_val(buffer) != *size as usize {
            return None;
        }

        Some(Self {
            elements: NonNull::new(buffer.as_mut_ptr()).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity: *size,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        })
    }

    /// Create a new exclusive instance from provided uninitialized memory buffer.
    ///
    /// `size` must be `0` since none of the elements are initialized yet, the whole buffer is used
    /// as capacity.
    ///
    /// Returns `None` if `size != 0` or if the buffer is larger than `u32::MAX` bytes.
    #[inline(always)]
    pub fn from_uninit(uninit: &'a mut [MaybeUninit<Element>], size: &'a mut u32) -> Option<Self> {
        if *size != 0 {
            return None;
        }
        let capacity = u32::try_from(size_of_val(uninit)).ok()?;

        Some(Self {
            elements: NonNull::new(uninit.as_mut_ptr().cast_init()).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity,
            memory: PhantomData,
            _supported: const { [(); NON_ZERO_SIZED::<Element>] },
        })
    }

    /// Number of elements
    #[inline(always)]
    pub const fn count(&self) -> u32 {
        self.size() / Element::SIZE
    }

    /// Try to get access to initialized elements
    #[inline(always)]
    pub const fn get_initialized(&self) -> &[Element] {
        let size = self.size();
        let ptr = self.elements.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts(ptr, (size / Element::SIZE) as usize) }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline(always)]
    pub fn get_initialized_mut(&mut self) -> &mut [Element] {
        let size = self.size();
        let ptr = self.elements.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts_mut(ptr, (size / Element::SIZE) as usize) }
    }

    /// Append some elements by using more of allocated, but currently unused elements.
    ///
    /// `true` is returned on success, but if there isn't enough unused elements left, `false` is.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, elements: &[Element]) -> bool {
        let size = self.size();
        let Some(new_size) = u32::try_from(size_of_val(elements))
            .ok()
            .and_then(|appended_size| size.checked_add(appended_size))
        else {
            return false;
        };
        if new_size > self.capacity {
            return false;
        }

        // May overflow, which is not allowed
        let Ok(offset) = isize::try_from(size / Element::SIZE) else {
            return false;
        };

        // SAFETY: allocation range and offset are checked above, the allocation itself is
        // guaranteed by constructors
        let start = unsafe { self.elements.offset(offset) };
        // SAFETY: Alignment is the same, writing happens in properly allocated memory guaranteed by
        // constructors, number of elements is checked above, Rust ownership rules will prevent any
        // overlap here (creating reference to non-initialized part of allocation would already be
        // undefined behavior anyway). The new size covers initialized elements only and is checked
        // to be within capacity above.
        unsafe {
            ptr::copy_nonoverlapping(elements.as_ptr(), start.as_ptr(), elements.len());
            self.size.write(new_size);
        }

        true
    }

    /// Truncate internal initialized bytes to this size.
    ///
    /// Returns `true` on success or `false` if `new_size` is larger than [`Self::size()`] or not a
    /// multiple of `Element::SIZE`.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn truncate(&mut self, new_size: u32) -> bool {
        if new_size > self.size() || !new_size.is_multiple_of(Element::SIZE) {
            return false;
        }

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(new_size);
        }

        true
    }

    /// Copy contents from another instance.
    ///
    /// Returns `false` if actual capacity of the instance is not enough to copy contents of `src`
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn copy_from(
        &mut self,
        src: &VariableElements<'_, Element, RECOMMENDED_ALLOCATION>,
    ) -> bool {
        let src_size = src.size();
        if src_size > self.capacity {
            return false;
        }

        // SAFETY: `src` can't be the same as `&mut self` if invariants of constructor arguments
        // were upheld, size is checked to be within capacity above, and the number of copied
        // elements corresponds to that size
        unsafe {
            self.elements
                .copy_from_nonoverlapping(src.elements, (src_size / Element::SIZE) as usize);
            self.size.write(src_size);
        }

        true
    }

    /// Assume that the first `size` are initialized and can be read.
    ///
    /// Returns `Some(initialized_elements)` on success or `None` if `size` is larger than its
    /// capacity or not a multiple of `Element::SIZE`.
    ///
    /// # Safety
    /// Caller must ensure `size` is actually initialized
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub unsafe fn assume_init(&mut self, size: u32) -> Option<&mut [Element]> {
        if size > self.capacity || !size.is_multiple_of(Element::SIZE) {
            return None;
        }

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
        Some(self.get_initialized_mut())
    }
}

// These methods don't convert sizes to the number of elements, so they work for any element type
impl<'a, Element, const RECOMMENDED_ALLOCATION: u32>
    VariableElements<'a, Element, RECOMMENDED_ALLOCATION>
where
    Element: TrivialType,
{
    // Size in bytes
    #[inline(always)]
    pub const fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    /// Capacity in bytes
    #[inline(always)]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Cast a shared reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline(always)]
    pub fn cast_ref<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &self,
    ) -> &VariableElements<'a, Element, DIFFERENT_RECOMMENDED_ALLOCATION> {
        // SAFETY: `VariableElements` has a fixed layout due to `#[repr(C)]`, which doesn't depend
        // on recommended allocation
        unsafe {
            NonNull::from_ref(self)
                .cast::<VariableElements<'a, Element, DIFFERENT_RECOMMENDED_ALLOCATION>>()
                .as_ref()
        }
    }

    /// Cast an exclusive reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline(always)]
    pub fn cast_mut<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &mut self,
    ) -> &mut VariableElements<'a, Element, DIFFERENT_RECOMMENDED_ALLOCATION> {
        // SAFETY: `VariableElements` has a fixed layout due to `#[repr(C)]`, which doesn't depend
        // on recommended allocation
        unsafe {
            NonNull::from_mut(self)
                .cast::<VariableElements<'a, Element, DIFFERENT_RECOMMENDED_ALLOCATION>>()
                .as_mut()
        }
    }
}
