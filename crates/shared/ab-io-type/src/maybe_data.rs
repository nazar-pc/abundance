#[cfg(test)]
mod tests;

use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::{NON_ZERO_SIZED, TrivialType};
use crate::{DerefWrapper, IoType, IoTypeOptional};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::ptr::NonNull;

/// Wrapper type for `Data` that may or may not be filled with contents.
///
/// This is somewhat similar to [`VariableElements`](crate::variable_elements::VariableElements),
/// but instead of a variable number of elements, data structure allows either having it or not
/// having the contents, which is a simpler and more appropriate API in many cases.
#[derive(Debug)]
pub struct MaybeData<Data>
where
    Data: TrivialType,
{
    data: NonNull<Data>,
    size: NonNull<u32>,
    // TODO: Technically redundant with the where-clause, which is not enforced in generic code
    //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed. Every instance
    //  evaluates the check through this field. It is initialized in `const` blocks, which are
    //  evaluated even if optimizations remove the field.
    _supported: [(); NON_ZERO_SIZED::<Data>],
}

// SAFETY: Low-level (effectively internal) implementation that upholds safety requirements
unsafe impl<Data> IoType for MaybeData<Data>
where
    Data: TrivialType,
    [(); NON_ZERO_SIZED::<Data>]:,
{
    const METADATA: &[u8] = {
        const fn metadata(inner_metadata: &[u8]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[&[IoTypeMetadataKind::MaybeData as u8], inner_metadata])
        }

        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
        metadata(Data::METADATA)
            .0
            .split_at(metadata(Data::METADATA).1)
            .0
    };

    type PointerType = Data;

    #[inline(always)]
    fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    #[inline(always)]
    fn capacity(&self) -> u32 {
        Data::SIZE
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size == 0 || size == Data::SIZE,
            "`set_size` called with invalid input {size} (self size {})",
            self.size()
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
            *size == 0 || *size <= capacity,
            "Invalid size {size} for capacity {capacity}"
        );
        // Read-only instance can have 0 capacity if empty
        debug_assert!(
            capacity == 0 || capacity >= Data::SIZE,
            "Invalid capacity {capacity} for size {}",
            Data::SIZE
        );

        let size = NonNull::from_ref(size);

        DerefWrapper(MaybeData {
            data: *ptr,
            size,
            _supported: const { [(); NON_ZERO_SIZED::<Data>] },
        })
    }

    #[inline(always)]
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size == 0 || *size <= capacity,
            "Invalid size {size} for capacity {capacity}"
        );
        debug_assert!(
            capacity >= Data::SIZE,
            "Invalid capacity {capacity} for size {}",
            Data::SIZE
        );

        DerefWrapper(MaybeData {
            data: *ptr,
            size: NonNull::from_mut(size),
            _supported: const { [(); NON_ZERO_SIZED::<Data>] },
        })
    }

    #[inline(always)]
    fn as_ptr(&self) -> NonNull<Self::PointerType> {
        self.data
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> NonNull<Self::PointerType> {
        self.data
    }
}

// SAFETY: Size `0` means data is missing, data is only accessed when the size is the size of data
unsafe impl<Data> IoTypeOptional for MaybeData<Data>
where
    Data: TrivialType,
    [(); NON_ZERO_SIZED::<Data>]:,
{
}

// Absence of data is represented by zero size, so `Data` itself must not be zero-sized
impl<Data> MaybeData<Data>
where
    Data: TrivialType,
    [(); NON_ZERO_SIZED::<Data>]:,
{
    /// Create a new shared instance from provided data reference.
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    pub const fn from_ref(data: Option<&'_ Data>) -> impl Deref<Target = Self> + '_ {
        let (data, size) = if let Some(data) = data {
            (NonNull::from_ref(data), &Data::SIZE)
        } else {
            (NonNull::dangling(), &0)
        };

        DerefWrapper(Self {
            data,
            size: NonNull::from_ref(size),
            _supported: const { [(); NON_ZERO_SIZED::<Data>] },
        })
    }

    /// Create a new exclusive instance from provided data reference.
    ///
    /// `size` can be either `0` or `Data::SIZE`, indicating that value is missing or present
    /// accordingly.
    ///
    /// Returns `None` if `size` is neither `0` nor `Data::SIZE`.
    //
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    pub fn from_mut<'a>(
        buffer: &'a mut Data,
        size: &'a mut u32,
    ) -> Option<impl DerefMut<Target = Self> + 'a> {
        if !(*size == 0 || *size == Data::SIZE) {
            return None;
        }

        Some(DerefWrapper(Self {
            data: NonNull::from_mut(buffer),
            size: NonNull::from_mut(size),
            _supported: const { [(); NON_ZERO_SIZED::<Data>] },
        }))
    }

    /// Create a new exclusive instance from provided uninitialized memory.
    ///
    /// `size` must be `0` since data is not initialized yet.
    ///
    /// Returns `None` if `size != 0`.
    //
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    pub fn from_uninit<'a>(
        uninit: &'a mut MaybeUninit<Data>,
        size: &'a mut u32,
    ) -> Option<impl DerefMut<Target = Self> + 'a> {
        if *size != 0 {
            return None;
        }

        Some(DerefWrapper(Self {
            data: NonNull::from_mut(uninit).cast::<Data>(),
            size: NonNull::from_mut(size),
            _supported: const { [(); NON_ZERO_SIZED::<Data>] },
        }))
    }

    /// Try to get access to initialized `Data`, returns `None` if not initialized
    #[inline(always)]
    pub const fn get(&self) -> Option<&Data> {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == Data::SIZE {
            // SAFETY: initialized
            Some(unsafe { self.data.as_ref() })
        } else {
            None
        }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline(always)]
    pub fn get_mut(&mut self) -> Option<&mut Data> {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == Data::SIZE {
            // SAFETY: initialized
            Some(unsafe { self.data.as_mut() })
        } else {
            None
        }
    }

    /// Initialize by inserting `Data` by value or replace existing value and return reference to it
    #[inline(always)]
    pub fn replace(&mut self, data: Data) -> &mut Data {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(Data::SIZE);
        }
        // SAFETY: constructor guarantees that memory is aligned
        unsafe {
            self.data.write(data);
            self.data.as_mut()
        }
    }

    /// Remove `Data` inside and turn instance back into uninitialized
    #[inline(always)]
    pub fn remove(&mut self) {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(0);
        }
    }

    /// Get exclusive access to initialized `Data`, running provided initialization function if
    /// necessary.
    ///
    /// `init` is expected to initialize provided memory and return a reference to it, like
    /// [`MaybeUninit::write()`] does. If it returns a reference to some other `Data` instead, that
    /// value is copied into this instance.
    #[inline(always)]
    pub fn get_mut_or_init_with<Init>(&mut self, init: Init) -> &mut Data
    where
        Init: FnOnce(&mut MaybeUninit<Data>) -> &mut Data,
    {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == Data::SIZE {
            // SAFETY: initialized
            unsafe { self.data.as_mut() }
        } else {
            // SAFETY: constructor guarantees that memory is aligned
            let data = init(unsafe { self.data.as_uninit_mut() });
            if !ptr::eq(data, self.data.as_ptr()) {
                // SAFETY: constructor guarantees that memory is aligned and valid for writes
                unsafe {
                    self.data.write(*data);
                }
            }
            // SAFETY: guaranteed to be initialized by constructors
            unsafe {
                self.size.write(Data::SIZE);
            }
            // SAFETY: Provided memory is initialized, either by `init`, which can only get a
            // reference to it by initializing it, or by the copy above
            unsafe { self.data.as_mut() }
        }
    }

    /// Assume value is initialized
    ///
    /// # Safety
    /// Caller must ensure `Data` is actually properly initialized
    #[inline(always)]
    pub unsafe fn assume_init(&mut self) -> &mut Data {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(Data::SIZE);
        }
        // SAFETY: guaranteed to be initialized by caller, the rest of guarantees are provided by
        // constructors
        unsafe { self.data.as_mut() }
    }
}

impl<Data> MaybeData<Data>
where
    Data: TrivialType + Default,
    [(); NON_ZERO_SIZED::<Data>]:,
{
    /// Get exclusive access to initialized `Data`, initializing with default value if necessary
    #[inline(always)]
    pub fn get_mut_or_default(&mut self) -> &mut Data {
        self.get_mut_or_init_with(|data| data.write(Data::default()))
    }
}
