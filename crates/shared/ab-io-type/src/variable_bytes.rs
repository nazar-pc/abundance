use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;
use crate::{DerefWrapper, IoType, IoTypeOptional};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{ptr, slice};

/// Container for storing variable number of bytes.
///
/// `RECOMMENDED_ALLOCATION` is what is being used when a host needs to allocate memory for call
/// into guest, but guest may receive an allocation with more or less memory in practice depending
/// on other circumstances, like when called from another contract with specific allocation
/// specified.
#[derive(Debug)]
#[repr(C)]
pub struct VariableBytes<const RECOMMENDED_ALLOCATION: u32 = 0> {
    bytes: NonNull<u8>,
    size: NonNull<u32>,
    capacity: u32,
}

// SAFETY: Low-level (effectively internal) implementation that upholds safety requirements
unsafe impl<const RECOMMENDED_ALLOCATION: u32> IoType for VariableBytes<RECOMMENDED_ALLOCATION> {
    const METADATA: &[u8] = {
        const fn metadata(recommended_allocation: u32) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            if recommended_allocation == 0 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes0 as u8]]);
            } else if recommended_allocation == 512 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes512 as u8]]);
            } else if recommended_allocation == 1024 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes1024 as u8]]);
            } else if recommended_allocation == 2028 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes2028 as u8]]);
            } else if recommended_allocation == 4096 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes4096 as u8]]);
            } else if recommended_allocation == 8192 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes8192 as u8]]);
            } else if recommended_allocation == 16384 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes16384 as u8]]);
            } else if recommended_allocation == 32768 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes32768 as u8]]);
            } else if recommended_allocation == 65536 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes65536 as u8]]);
            } else if recommended_allocation == 131_072 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes131072 as u8],
                ]);
            } else if recommended_allocation == 262_144 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes262144 as u8],
                ]);
            } else if recommended_allocation == 524_288 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes524288 as u8],
                ]);
            } else if recommended_allocation == 1_048_576 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes1048576 as u8
                ]]);
            }

            let (io_type, size_bytes) = if recommended_allocation < 2u32.pow(8) {
                (IoTypeMetadataKind::VariableBytes8b, 1)
            } else if recommended_allocation < 2u32.pow(16) {
                (IoTypeMetadataKind::VariableBytes16b, 2)
            } else {
                (IoTypeMetadataKind::VariableBytes32b, 4)
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

    // TODO: Use `[u8; RECOMMENDED_ALLOCATION as usize]` once stabilized `generic_const_exprs`
    //  allows us to do so
    type PointerType = u8;

    #[inline(always)]
    fn size(&self) -> u32 {
        self.size()
    }

    #[inline(always)]
    unsafe fn size_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(self.size)
    }

    #[inline(always)]
    unsafe fn size_mut_ptr(&mut self) -> impl DerefMut<Target = *mut u32> {
        DerefWrapper(self.size.as_ptr())
    }

    #[inline(always)]
    fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline(always)]
    unsafe fn capacity_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&self.capacity))
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size <= self.capacity,
            "`set_size` called with invalid input {size} for capacity {}",
            self.capacity
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
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );

        DerefWrapper(Self {
            bytes: *ptr,
            size: NonNull::from_ref(size),
            capacity,
        })
    }

    #[inline(always)]
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
            // SAFETY: Must be guaranteed by the caller
            let size = unsafe { size.read() };
            debug_assert!(
                size <= capacity,
                "Size {size} must not exceed capacity {capacity}"
            );
        }

        DerefWrapper(Self {
            bytes: *ptr,
            size,
            capacity,
        })
    }

    #[inline(always)]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        &self.bytes
    }

    #[inline(always)]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        &mut self.bytes
    }
}

impl<const RECOMMENDED_ALLOCATION: u32> IoTypeOptional for VariableBytes<RECOMMENDED_ALLOCATION> {}

impl<const RECOMMENDED_ALLOCATION: u32> VariableBytes<RECOMMENDED_ALLOCATION> {
    /// Create a new shared instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `buffer.len() != size`
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    #[inline(always)]
    #[track_caller]
    pub const fn from_buffer<'a>(
        buffer: &'a [<Self as IoType>::PointerType],
        size: &'a u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(buffer.len() == *size as usize, "Invalid size");
        // TODO: Use `debug_assert_eq` when it is available in const environment
        // debug_assert_eq!(buffer.len(), *size as usize, "Invalid size");

        DerefWrapper(Self {
            bytes: NonNull::new(buffer.as_ptr().cast_mut()).expect("Not null; qed"),
            size: NonNull::from_ref(size),
            capacity: *size,
        })
    }

    /// Create a new exclusive instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `buffer.len() != size`
    //
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    #[inline(always)]
    #[track_caller]
    pub fn from_buffer_mut<'a>(
        buffer: &'a mut [<Self as IoType>::PointerType],
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert_eq!(buffer.len(), *size as usize, "Invalid size");

        DerefWrapper(Self {
            bytes: NonNull::new(buffer.as_mut_ptr()).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity: *size,
        })
    }

    /// Create a new shared instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `size > CAPACITY`
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    #[inline(always)]
    #[track_caller]
    pub fn from_uninit<'a>(
        uninit: &'a mut [MaybeUninit<<Self as IoType>::PointerType>],
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        let capacity = uninit.len();
        debug_assert!(
            *size as usize <= capacity,
            "Size {size} must not exceed capacity {capacity}"
        );
        let capacity = capacity as u32;

        DerefWrapper(Self {
            bytes: NonNull::new(MaybeUninit::slice_as_mut_ptr(uninit)).expect("Not null; qed"),
            size: NonNull::from_mut(size),
            capacity,
        })
    }

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

    /// Try to get access to initialized bytes
    #[inline(always)]
    pub const fn get_initialized(&self) -> &[u8] {
        let size = self.size();
        let ptr = self.bytes.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts(ptr, size as usize) }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline(always)]
    pub fn get_initialized_mut(&mut self) -> &mut [u8] {
        let size = self.size();
        let ptr = self.bytes.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts_mut(ptr, size as usize) }
    }

    /// Append some bytes by using more of allocated, but currently unused bytes.
    ///
    /// `true` is returned on success, but if there isn't enough unused bytes left, `false` is.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let size = self.size();
        if bytes.len() + size as usize > self.capacity as usize {
            return false;
        }

        // May overflow, which is not allowed
        let Ok(offset) = isize::try_from(size) else {
            return false;
        };

        // SAFETY: allocation range and offset are checked above, the allocation itself is
        // guaranteed by constructors
        let mut start = unsafe { self.bytes.offset(offset) };
        // SAFETY: Alignment is the same, writing happens in properly allocated memory guaranteed by
        // constructors, number of bytes is checked above, Rust ownership rules will prevent any
        // overlap here (creating reference to non-initialized part of allocation would already be
        // undefined behavior anyway)
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), start.as_mut(), bytes.len()) }

        true
    }

    /// Truncate internal initialized bytes to this size.
    ///
    /// Returns `true` on success or `false` if `new_size` is larger than [`Self::size()`].
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn truncate(&mut self, new_size: u32) -> bool {
        if new_size > self.size() {
            return false;
        }

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(new_size);
        }

        true
    }

    /// Copy contents from another `IoType`.
    ///
    /// Returns `false` if actual capacity of the instance is not enough to copy contents of `src`
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn copy_from<T>(&mut self, src: &T) -> bool
    where
        T: IoType,
    {
        let src_size = src.size();
        if src_size > self.capacity {
            return false;
        }

        // SAFETY: `src` can't be the same as `&mut self` if invariants of constructor arguments
        // were upheld, size is checked to be within capacity above
        unsafe {
            self.bytes
                .copy_from_nonoverlapping(src.as_ptr().cast::<u8>(), src_size as usize);
            self.size.write(src_size);
        }

        true
    }

    /// Get exclusive access to the underlying pointer with no checks.
    ///
    /// Can be used for initialization with [`Self::assume_init()`] called afterward to confirm how
    /// many bytes are in use right now.
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> &mut NonNull<u8> {
        &mut self.bytes
    }

    /// Cast a shared reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline(always)]
    pub fn cast_ref<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &self,
    ) -> &VariableBytes<DIFFERENT_RECOMMENDED_ALLOCATION> {
        // SAFETY: `VariableBytes` has a fixed layout due to `#[repr(C)]`, which doesn't depend on
        // recommended allocation
        unsafe {
            NonNull::from_ref(self)
                .cast::<VariableBytes<DIFFERENT_RECOMMENDED_ALLOCATION>>()
                .as_ref()
        }
    }

    /// Cast an exclusive reference to this instance into a reference to an instance of a different
    /// recommended allocation
    #[inline(always)]
    pub fn cast_mut<const DIFFERENT_RECOMMENDED_ALLOCATION: u32>(
        &mut self,
    ) -> &mut VariableBytes<DIFFERENT_RECOMMENDED_ALLOCATION> {
        // SAFETY: `VariableBytes` has a fixed layout due to `#[repr(C)]`, which doesn't depend on
        // recommended allocation
        unsafe {
            NonNull::from_mut(self)
                .cast::<VariableBytes<DIFFERENT_RECOMMENDED_ALLOCATION>>()
                .as_mut()
        }
    }

    /// Reads and returns value of type `T` or `None` if there is not enough data.
    ///
    /// Checks alignment internally to support both aligned and unaligned reads.
    #[inline(always)]
    pub fn read_trivial_type<T>(&self) -> Option<T>
    where
        T: TrivialType,
    {
        if self.size() < T::SIZE {
            return None;
        }

        let ptr = self.bytes.cast::<T>();

        // SAFETY: Trivial types are safe to read as bytes, pointer validity is a guaranteed
        // internal invariant
        let value = unsafe {
            if ptr.is_aligned() {
                ptr.read()
            } else {
                ptr.read_unaligned()
            }
        };

        Some(value)
    }

    /// Assume that the first `size` are initialized and can be read.
    ///
    /// Returns `Some(initialized_bytes)` on success or `None` if `size` is larger than its
    /// capacity.
    ///
    /// # Safety
    /// Caller must ensure `size` is actually initialized
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub unsafe fn assume_init(&mut self, size: u32) -> Option<&mut [u8]> {
        if size > self.capacity {
            return None;
        }

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
        Some(self.get_initialized_mut())
    }
}
