use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;
use crate::{DerefWrapper, IoType, IoTypeOptional};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{ptr, slice};

/// Container for storing variable number of elements.
///
/// `RECOMMENDED_ALLOCATION` is what is being used when a host needs to allocate memory for call
/// into guest, but guest may receive an allocation with more or less memory in practice depending
/// on other circumstances, like when called from another contract with specific allocation
/// specified.
#[derive(Debug)]
#[repr(C)]
pub struct VariableElements<const RECOMMENDED_ALLOCATION: u32, Element>
where
    Element: TrivialType,
{
    elements: NonNull<Element>,
    size: NonNull<u32>,
    capacity: u32,
}

unsafe impl<const RECOMMENDED_ALLOCATION: u32, Element> IoType
    for VariableElements<RECOMMENDED_ALLOCATION, Element>
where
    Element: TrivialType,
{
    const METADATA: &[u8] = {
        const fn metadata(recommended_allocation: u32) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            if recommended_allocation == 0 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableElements0 as u8]]);
            }

            let (io_type, size_bytes) = if recommended_allocation < 2u32.pow(8) {
                (IoTypeMetadataKind::VariableElements8b, 1)
            } else if recommended_allocation < 2u32.pow(16) {
                (IoTypeMetadataKind::VariableElements16b, 2)
            } else {
                (IoTypeMetadataKind::VariableElements32b, 4)
            };

            concat_metadata_sources(&[
                &[io_type as u8],
                recommended_allocation.to_le_bytes().split_at(size_bytes).0,
            ])
        }

        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
        metadata(RECOMMENDED_ALLOCATION)
            .0
            .split_at(metadata(RECOMMENDED_ALLOCATION).1)
            .0
    };

    // TODO: Use `[Element; RECOMMENDED_ALLOCATION as usize]` once stabilized `generic_const_exprs`
    //  allows us to do so
    type PointerType = Element;

    #[inline]
    fn size(&self) -> u32 {
        self.size()
    }

    #[inline]
    unsafe fn size_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(self.size)
    }

    #[inline]
    unsafe fn size_mut_ptr(&mut self) -> impl DerefMut<Target = *mut u32> {
        DerefWrapper(self.size.as_ptr())
    }

    #[inline]
    fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline]
    unsafe fn capacity_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&self.capacity))
    }

    #[inline]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size <= self.capacity,
            "`set_size` called with invalid input {size} for capacity {}",
            self.capacity
        );
        debug_assert_eq!(
            size % Element::SIZE,
            0,
            "`set_size` called with invalid input {size} for element size {}",
            Element::SIZE
        );

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        debug_assert_eq!(
            size % Element::SIZE,
            0,
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );

        DerefWrapper(Self {
            elements: *ptr,
            size: NonNull::from_ref(size),
            capacity,
        })
    }

    #[inline]
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut *mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(!size.is_null(), "`null` pointer for non-`TrivialType` size");
        // SAFETY: Must be guaranteed by the caller + debug check above
        let size = unsafe { NonNull::new_unchecked(*size) };
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        {
            let size = unsafe { size.read() };
            debug_assert!(
                size <= capacity,
                "Size {size} must not exceed capacity {capacity}"
            );
            debug_assert_eq!(
                size % Element::SIZE,
                0,
                "Size {size} is invalid for element size {}",
                Element::SIZE
            );
        }

        DerefWrapper(Self {
            elements: *ptr,
            size,
            capacity,
        })
    }

    #[inline]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        &self.elements
    }

    #[inline]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        &mut self.elements
    }
}

impl<const RECOMMENDED_ALLOCATION: u32, Element> IoTypeOptional
    for VariableElements<RECOMMENDED_ALLOCATION, Element>
where
    Element: TrivialType,
{
}

impl<const RECOMMENDED_ALLOCATION: u32, Element> VariableElements<RECOMMENDED_ALLOCATION, Element>
where
    Element: TrivialType,
{
    /// Create a new shared instance from provided memory buffer.
    ///
    /// NOTE: size is specified in bytes, not elements.
    ///
    /// # Panics
    /// Panics if `buffer.len * Element::SIZE() != size`
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    #[track_caller]
    pub const fn from_buffer<'a>(
        buffer: &'a [<Self as IoType>::PointerType],
        size: &'a u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(
            buffer.len() * Element::SIZE as usize == *size as usize,
            "Invalid size"
        );
        // TODO: Use `debug_assert_eq` when it is available in const environment
        // debug_assert_eq!(buffer.len(), *size as usize, "Invalid size");

        DerefWrapper(Self {
            elements: NonNull::new(buffer.as_ptr().cast_mut()).expect("Not null; qed"),
            size: NonNull::from_ref(size),
            capacity: *size,
        })
    }

    /// Create a new exclusive instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `buffer.len() * Element::SIZE != size`
    //
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    #[track_caller]
    pub fn from_buffer_mut<'a>(
        buffer: &'a mut [<Self as IoType>::PointerType],
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert_eq!(
            buffer.len() * Element::SIZE as usize,
            *size as usize,
            "Invalid size"
        );

        DerefWrapper(Self {
            elements: NonNull::new(buffer.as_mut_ptr()).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity: *size,
        })
    }

    /// Create a new shared instance from provided memory buffer.
    ///
    /// NOTE: size is specified in bytes, not elements.
    ///
    /// # Panics
    /// Panics if `size > CAPACITY` or `size % Element::SIZE != 0`
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    // TODO: Change `usize` to `u32` once stabilized `generic_const_exprs` feature allows us to do
    //  `CAPACITY as usize`
    #[track_caller]
    pub fn from_uninit<'a, const CAPACITY: usize>(
        uninit: &'a mut [MaybeUninit<<Self as IoType>::PointerType>; CAPACITY],
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(
            *size as usize <= CAPACITY,
            "Size {size} must not exceed capacity {CAPACITY}"
        );
        debug_assert_eq!(
            *size % Element::SIZE,
            0,
            "Size {size} is invalid for element size {}",
            Element::SIZE
        );
        let capacity = CAPACITY as u32;

        DerefWrapper(Self {
            elements: NonNull::new(MaybeUninit::slice_as_mut_ptr(uninit)).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity,
        })
    }

    // Size in bytes
    #[inline]
    pub const fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    /// Capacity in bytes
    #[inline]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Number of elements
    #[inline]
    pub const fn count(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    /// Try to get access to initialized elements
    #[inline]
    pub const fn get_initialized(&self) -> &[Element] {
        let size = self.size();
        let ptr = self.elements.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts(ptr, (size / Element::SIZE) as usize) }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub fn get_initialized_mut(&mut self) -> &mut [Element] {
        let size = self.size();
        let ptr = self.elements.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts_mut(ptr, (size / Element::SIZE) as usize) }
    }

    /// Append some elements by using more of allocated, but currently unused elements.
    ///
    /// `true` is returned on success, but if there isn't enough unused elements left, `false` is.
    #[inline]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, elements: &[Element]) -> bool {
        let size = self.size();
        if elements.len() * Element::SIZE as usize + size as usize > self.capacity as usize {
            return false;
        }

        // May overflow, which is not allowed
        let Ok(offset) = isize::try_from(size / Element::SIZE) else {
            return false;
        };

        // SAFETY: allocation range and offset are checked above, the allocation itself is
        // guaranteed by constructors
        let mut start = unsafe { self.elements.offset(offset) };
        // SAFETY: Alignment is the same, writing happens in properly allocated memory guaranteed by
        // constructors, number of elements is checked above, Rust ownership rules will prevent any
        // overlap here (creating reference to non-initialized part of allocation would already be
        // undefined behavior anyway)
        unsafe { ptr::copy_nonoverlapping(elements.as_ptr(), start.as_mut(), elements.len()) }

        true
    }

    /// Truncate internal initialized bytes to this size.
    ///
    /// Returns `true` on success or `false` if `new_size` is larger than [`Self::size()`] or not a
    /// multiple of `Element::SIZE`.
    #[inline]
    #[must_use = "Operation may fail"]
    pub fn truncate(&mut self, new_size: u32) -> bool {
        if new_size > self.size() || new_size % Element::SIZE != 0 {
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
    #[inline]
    #[must_use = "Operation may fail"]
    pub fn copy_from(&mut self, src: &Self) -> bool {
        let src_size = src.size();
        if src_size > self.capacity {
            return false;
        }

        // Safety: `src` can't be the same as `&mut self` if invariants of constructor arguments
        // were upheld, size is checked to be within capacity above
        unsafe {
            self.elements
                .copy_from_nonoverlapping(src.elements, src_size as usize);
            self.size.write(src_size);
        }

        true
    }

    /// Get exclusive access to the underlying pointer with no checks.
    ///
    /// Can be used for initialization with [`Self::assume_init()`] called afterward to confirm how
    /// many bytes are in use right now.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> &mut NonNull<Element> {
        &mut self.elements
    }

    /// Cast a shared reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline]
    pub fn cast_ref<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &self,
    ) -> &VariableElements<DIFFERENT_RECOMMENDED_ALLOCATION, Element> {
        // SAFETY: `VariableElements` has a fixed layout due to `#[repr(C)]`, which doesn't depend
        // on recommended allocation
        unsafe {
            NonNull::from_ref(self)
                .cast::<VariableElements<DIFFERENT_RECOMMENDED_ALLOCATION, Element>>()
                .as_ref()
        }
    }

    /// Cast an exclusive reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline]
    pub fn cast_mut<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &mut self,
    ) -> &mut VariableElements<DIFFERENT_RECOMMENDED_ALLOCATION, Element> {
        // SAFETY: `VariableElements` has a fixed layout due to `#[repr(C)]`, which doesn't depend
        // on recommended allocation
        unsafe {
            NonNull::from_mut(self)
                .cast::<VariableElements<DIFFERENT_RECOMMENDED_ALLOCATION, Element>>()
                .as_mut()
        }
    }

    /// Assume that the first `size` are initialized and can be read.
    ///
    /// Returns `Some(initialized_elements)` on success or `None` if `size` is larger than its
    /// capacity or not a multiple of `Element::SIZE`.
    ///
    /// # Safety
    /// Caller must ensure `size` is actually initialized
    #[inline]
    #[must_use = "Operation may fail"]
    pub unsafe fn assume_init(&mut self, size: u32) -> Option<&mut [Element]> {
        if size > self.capacity || size % Element::SIZE != 0 {
            return None;
        }

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
        Some(self.get_initialized_mut())
    }
}
