use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
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
pub struct VariableBytes<const RECOMMENDED_ALLOCATION: u32> {
    bytes: NonNull<u8>,
    size: NonNull<u32>,
    capacity: u32,
}

unsafe impl<const RECOMMENDED_ALLOCATION: u32> IoType for VariableBytes<RECOMMENDED_ALLOCATION> {
    const METADATA: &[u8] = {
        const fn metadata(max_capacity: u32) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            if max_capacity == 512 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes512 as u8]]);
            } else if max_capacity == 1024 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes1024 as u8]]);
            } else if max_capacity == 2028 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes2028 as u8]]);
            } else if max_capacity == 4096 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes4096 as u8]]);
            } else if max_capacity == 8192 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes8192 as u8]]);
            } else if max_capacity == 16384 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes16384 as u8]]);
            } else if max_capacity == 32768 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes32768 as u8]]);
            } else if max_capacity == 65536 {
                return concat_metadata_sources(&[&[IoTypeMetadataKind::VariableBytes65536 as u8]]);
            } else if max_capacity == 131072 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes131072 as u8],
                ]);
            } else if max_capacity == 262144 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes262144 as u8],
                ]);
            } else if max_capacity == 524288 {
                return concat_metadata_sources(&[
                    &[IoTypeMetadataKind::VariableBytes524288 as u8],
                ]);
            } else if max_capacity == 1048576 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes1048576 as u8
                ]]);
            } else if max_capacity == 2097152 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes2097152 as u8
                ]]);
            } else if max_capacity == 4194304 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes4194304 as u8
                ]]);
            } else if max_capacity == 8388608 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes8388608 as u8
                ]]);
            } else if max_capacity == 16777216 {
                return concat_metadata_sources(&[&[
                    IoTypeMetadataKind::VariableBytes16777216 as u8
                ]]);
            }

            let (io_type, size_bytes) = if max_capacity < 2u32.pow(8) {
                (IoTypeMetadataKind::VariableBytes8b, 1)
            } else if max_capacity < 2u32.pow(16) {
                (IoTypeMetadataKind::VariableBytes16b, 2)
            } else {
                (IoTypeMetadataKind::VariableBytes32b, 4)
            };

            concat_metadata_sources(&[
                &[io_type as u8],
                max_capacity.to_le_bytes().split_at(size_bytes).0,
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

    #[inline]
    fn size(&self) -> u32 {
        self.size()
    }

    #[inline]
    unsafe fn size_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(self.size)
    }

    #[inline]
    unsafe fn size_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<u32>> {
        DerefWrapper(self.size)
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
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size <= self.capacity,
            "`set_size` called with invalid input"
        );

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
    }

    #[inline]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(*size <= capacity, "Size larger than capacity");

        DerefWrapper(Self {
            bytes: *ptr,
            size: NonNull::from_ref(size),
            capacity,
        })
    }

    #[inline]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(*size <= capacity, "Size larger than capacity");

        DerefWrapper(Self {
            bytes: *ptr,
            size: NonNull::from_mut(size),
            capacity,
        })
    }

    #[inline]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        &self.bytes
    }

    #[inline]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        &mut self.bytes
    }
}

impl<const RECOMMENDED_ALLOCATION: u32> IoTypeOptional for VariableBytes<RECOMMENDED_ALLOCATION> {}

impl<const RECOMMENDED_ALLOCATION: u32> VariableBytes<RECOMMENDED_ALLOCATION> {
    /// Create a new shared instance from provided memory buffer.
    ///
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    pub fn from_buffer(buffer: &[<Self as IoType>::PointerType]) -> impl Deref<Target = Self> + '_ {
        let size = buffer.len() as u32;
        let capacity = size;

        DerefWrapper(Self {
            bytes: NonNull::from_ref(buffer).cast::<<Self as IoType>::PointerType>(),
            size: NonNull::from_ref(&size),
            capacity,
        })
    }

    /// Create a new exclusive instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `buffer.len() != size`
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    pub fn from_buffer_mut<'a>(
        buffer: &'a mut [<Self as IoType>::PointerType],
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(buffer.len() == *size as usize, "Invalid size");
        let capacity = *size;

        DerefWrapper(Self {
            bytes: NonNull::from_mut(buffer).cast::<<Self as IoType>::PointerType>(),
            size: NonNull::from_mut(size),
            capacity,
        })
    }

    /// Create a new shared instance from provided memory buffer.
    ///
    /// # Panics
    /// Panics if `size > SIZE`
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    // TODO: Change `usize` to `u32` once stabilized `generic_const_exprs` feature allows us to do
    //  `CAPACITY as usize`
    pub fn from_uninit<'a, const CAPACITY: usize>(
        uninit: &'a mut MaybeUninit<[<Self as IoType>::PointerType; CAPACITY]>,
        size: &'a mut u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(*size as usize <= CAPACITY, "Size larger than capacity");
        let capacity = CAPACITY as u32;

        DerefWrapper(Self {
            bytes: NonNull::from_mut(uninit).cast::<<Self as IoType>::PointerType>(),
            size: NonNull::from_mut(size),
            capacity,
        })
    }

    #[inline]
    pub fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    #[inline]
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Try to get access to initialized bytes
    #[inline]
    pub fn get_initialized(&self) -> &[u8] {
        let size = self.size();
        let ptr = self.bytes.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts(ptr, size as usize) }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub fn get_initialized_mut(&mut self) -> &mut [u8] {
        let size = self.size();
        let ptr = self.bytes.as_ptr();
        // SAFETY: guaranteed by constructor and explicit methods by the user
        unsafe { slice::from_raw_parts_mut(ptr, size as usize) }
    }

    /// Append some bytes by using more of allocated, but currently unused bytes.
    ///
    /// `true` is returned on success, but if there isn't enough unused bytes left, `false` is.
    #[inline]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let size = self.size();
        if bytes.len() as u32 > size + self.capacity {
            return false;
        }

        // May overflow, which is not allowed
        let Ok(offset) = isize::try_from(size) else {
            return false;
        };

        // SAFETY: allocation range and offset are checked above, the allocation itself is
        // guaranteed by constructors
        let mut start = unsafe { self.bytes.offset(offset) };
        // SAFETY: Alignment is 1, writing happens in properly allocated memory guaranteed by
        // constructors, number of bytes is checked above, Rust ownership rules will prevent any
        // overlap here (creating reference to non-initialized part of allocation would already be
        // undefined behavior anyway)
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), start.as_mut(), bytes.len()) }

        true
    }

    /// Truncate internal initialized bytes to this size.
    ///
    /// Returns `true` on success or `false` if `new_size` is larger than
    /// [`Self::size()`].
    #[inline]
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
            self.bytes
                .copy_from_nonoverlapping(src.bytes, src_size as usize);
            self.size.write(src_size);
        }

        true
    }

    /// Get exclusive access to underlying pointer with no checks.
    ///
    /// Can be used for initialization with [`Self::assume_init()`] called afterward to confirm how
    /// many bytes are in use right now.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> &mut NonNull<u8> {
        &mut self.bytes
    }

    /// Assume that the first `size` are initialized and can be read.
    ///
    /// Returns `Some(initialized_bytes)` on success or `None` if `size` is larger than its
    /// capacity.
    ///
    /// # Safety
    /// Caller must ensure `size` are actually initialized
    #[inline]
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
