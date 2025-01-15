use crate::utils::concat_metadata_sources;
use crate::{IoType, IoTypeMetadata, IoTypeOptional};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{ptr, slice};

struct VariableBytesWrapper<const CAPACITY: u32>(VariableBytes<CAPACITY>);

impl<const CAPACITY: u32> Deref for VariableBytesWrapper<CAPACITY> {
    type Target = VariableBytes<CAPACITY>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const CAPACITY: u32> DerefMut for VariableBytesWrapper<CAPACITY> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Container for storing variable number of bytes.
///
/// The total allocation is specified by `CAPACITY` constant generic, but actual number of bytes in
/// use can vary between `0` and `CAPACITY`. This is useful to minimize amount of data persisted in
/// the state, while keep host/guest API dealing with fixed size types and avoid dynamic allocations
/// on the heap in most cases.
pub struct VariableBytes<const CAPACITY: u32> {
    bytes: NonNull<u8>,
    size: NonNull<u32>,
}

unsafe impl<const CAPACITY: u32> IoType for VariableBytes<CAPACITY> {
    const CAPACITY: u32 = CAPACITY;
    const METADATA: &[u8] = {
        const fn metadata(size: u32) -> ([u8; 4096], usize) {
            if size == 512 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes512 as u8]]);
            } else if size == 1024 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes1024 as u8]]);
            } else if size == 2028 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes2028 as u8]]);
            } else if size == 4096 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes4096 as u8]]);
            } else if size == 8192 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes8192 as u8]]);
            } else if size == 16384 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes16384 as u8]]);
            } else if size == 32768 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes32768 as u8]]);
            } else if size == 65536 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes65536 as u8]]);
            } else if size == 131072 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes131072 as u8]]);
            } else if size == 262144 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes262144 as u8]]);
            } else if size == 524288 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes524288 as u8]]);
            } else if size == 1048576 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes1048576 as u8]]);
            } else if size == 4194304 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes4194304 as u8]]);
            } else if size == 8388608 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes8388608 as u8]]);
            } else if size == 16777216 {
                return concat_metadata_sources(&[&[IoTypeMetadata::VariableBytes16777216 as u8]]);
            }

            let (io_type, size_bytes) = if size < 2u32.pow(8) {
                (IoTypeMetadata::VariableBytes8b, 1)
            } else if size < 2u32.pow(16) {
                (IoTypeMetadata::VariableBytes16b, 2)
            } else {
                (IoTypeMetadata::VariableBytes32b, 4)
            };

            concat_metadata_sources(&[&[io_type as u8], size.to_le_bytes().split_at(size_bytes).0])
        }

        // Strange syntax to allow Rust to extend lifetime of metadata scratch automatically
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };

    // TODO: Use `[u8; CAPACITY as usize]` once stabilized `generic_const_exprs` allows us to do so
    type PointerType = u8;

    #[inline]
    fn size(&self) -> u32 {
        self.size()
    }

    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(size <= CAPACITY, "`set_size` called with invalid input");

        // SAFETY: guaranteed to be initialized by constructors
        self.size.write(size);
    }

    #[inline]
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned());
        debug_assert!(*size <= CAPACITY);

        VariableBytesWrapper(Self {
            bytes: *ptr,
            // TODO: Use `NonNull::from_ref()` once stable
            size: NonNull::from(size),
        })
    }

    #[inline]
    unsafe fn from_ptr_mut<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned());
        debug_assert!(*size <= CAPACITY);

        VariableBytesWrapper(Self {
            bytes: *ptr,
            // TODO: Use `NonNull::from_ref()` once stable
            size: NonNull::from(size),
        })
    }
}

impl<const CAPACITY: u32> IoTypeOptional for VariableBytes<CAPACITY> {
    #[inline]
    fn as_mut_ptr(&mut self) -> &mut NonNull<Self::PointerType> {
        &mut self.bytes
    }
}

impl<const CAPACITY: u32> VariableBytes<CAPACITY> {
    #[inline]
    pub fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
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
        if bytes.len() as u32 > size + CAPACITY {
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
    /// Returns `Some(initialized_bytes)` on success or `None` if `size` is larger than
    /// `CAPACITY`.
    ///
    /// # Safety
    /// Caller must ensure `size` are actually initialized
    #[inline]
    pub unsafe fn assume_init(&mut self, size: u32) -> Option<&mut [u8]> {
        if size > CAPACITY {
            return None;
        }

        // SAFETY: guaranteed to be initialized by constructors
        self.size.write(size);
        Some(self.get_initialized_mut())
    }
}
