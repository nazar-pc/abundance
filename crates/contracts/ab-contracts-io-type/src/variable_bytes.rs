use crate::utils::concat_metadata_sources;
use crate::{IoType, IoTypeMetadata, IoTypeOptional};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{ptr, slice};

struct VariableBytesWrapper<const RECOMMENDED_ALLOCATION: u32>(
    VariableBytes<RECOMMENDED_ALLOCATION>,
);

impl<const RECOMMENDED_ALLOCATION: u32> Deref for VariableBytesWrapper<RECOMMENDED_ALLOCATION> {
    type Target = VariableBytes<RECOMMENDED_ALLOCATION>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const RECOMMENDED_ALLOCATION: u32> DerefMut for VariableBytesWrapper<RECOMMENDED_ALLOCATION> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Container for storing variable number of bytes.
///
/// `RECOMMENDED_ALLOCATION` is what is being used when host needs to allocate memory for call into
/// guest, but guest may receive an allocation with more or less memory in practice depending on
/// other circumstances, like when called from another contract with specific allocation specified.
pub struct VariableBytes<const RECOMMENDED_ALLOCATION: u32> {
    bytes: NonNull<u8>,
    size: NonNull<u32>,
    capacity: u32,
}

unsafe impl<const RECOMMENDED_ALLOCATION: u32> IoType for VariableBytes<RECOMMENDED_ALLOCATION> {
    const METADATA: &[u8] = {
        const fn metadata(max_capacity: u32) -> ([u8; 4096], usize) {
            if max_capacity == 512 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes512 as u8]]);
            } else if max_capacity == 1024 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes1024 as u8]]);
            } else if max_capacity == 2028 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes2028 as u8]]);
            } else if max_capacity == 4096 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes4096 as u8]]);
            } else if max_capacity == 8192 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes8192 as u8]]);
            } else if max_capacity == 16384 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes16384 as u8]]);
            } else if max_capacity == 32768 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes32768 as u8]]);
            } else if max_capacity == 65536 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes65536 as u8]]);
            } else if max_capacity == 131072 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes131072 as u8]]);
            } else if max_capacity == 262144 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes262144 as u8]]);
            } else if max_capacity == 524288 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes524288 as u8]]);
            } else if max_capacity == 1048576 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes1048576 as u8]]);
            } else if max_capacity == 4194304 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes4194304 as u8]]);
            } else if max_capacity == 8388608 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes8388608 as u8]]);
            } else if max_capacity == 16777216 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes16777216 as u8]]);
            }

            let (io_type, size_bytes) = if max_capacity < 2u32.pow(8) {
                (IoTypeMetadata::VariableBytes8b, 1)
            } else if max_capacity < 2u32.pow(16) {
                (IoTypeMetadata::VariableBytes16b, 2)
            } else {
                (IoTypeMetadata::VariableBytes32b, 4)
            };

            concat_metadata_sources(&[
                &[io_type as u8],
                max_capacity.to_le_bytes().split_at(size_bytes).0,
            ])
        }

        // Strange syntax to allow Rust to extend lifetime of metadata scratch automatically
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
    fn capacity(&self) -> u32 {
        self.capacity
    }

    #[inline]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size <= self.capacity,
            "`set_size` called with invalid input"
        );

        // SAFETY: guaranteed to be initialized by constructors
        self.size.write(size);
    }

    #[inline]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(*size <= capacity, "Size larger than capacity");

        VariableBytesWrapper(Self {
            bytes: *ptr,
            // TODO: Use `NonNull::from_ref()` once stable
            size: NonNull::from(size),
            capacity,
        })
    }

    #[inline]
    unsafe fn from_ptr_mut<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(*size <= capacity, "Size larger than capacity");

        VariableBytesWrapper(Self {
            bytes: *ptr,
            // TODO: Use `NonNull::from_ref()` once stable
            size: NonNull::from(size),
            capacity,
        })
    }
}

impl<const RECOMMENDED_ALLOCATION: u32> IoTypeOptional for VariableBytes<RECOMMENDED_ALLOCATION> {
    #[inline]
    fn as_mut_ptr(&mut self) -> &mut NonNull<Self::PointerType> {
        &mut self.bytes
    }
}

impl<const RECOMMENDED_ALLOCATION: u32> VariableBytes<RECOMMENDED_ALLOCATION> {
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
        self.size.write(size);
        Some(self.get_initialized_mut())
    }
}
