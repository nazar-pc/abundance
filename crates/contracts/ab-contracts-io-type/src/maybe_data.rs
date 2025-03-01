use crate::trivial_type::TrivialType;
use crate::{DerefWrapper, IoType, IoTypeOptional};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

/// Wrapper type for `Data` that may or may not be filled with contents.
///
/// This is somewhat similar to [`VariableBytes`](crate::variable_bytes::VariableBytes), but instead
/// of variable size data structure allows to either have it or not have the contents or not have
/// it, which is a simpler and more convenient API that is also sufficient in many cases.
#[derive(Debug)]
pub struct MaybeData<Data>
where
    Data: TrivialType,
{
    data: NonNull<Data>,
    size: NonNull<u32>,
    capacity: u32,
}

unsafe impl<Data> IoType for MaybeData<Data>
where
    Data: TrivialType,
{
    const METADATA: &[u8] = Data::METADATA;

    type PointerType = Data;

    #[inline]
    fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
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
        self.size()
    }

    #[inline]
    unsafe fn capacity_ptr(&self) -> impl Deref<Target = NonNull<u32>> {
        DerefWrapper(NonNull::from_ref(&self.capacity))
    }

    #[inline]
    #[track_caller]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size == 0 || size == self.size(),
            "`set_size` called with invalid input {size} (self size {})",
            self.size()
        );

        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(size);
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn from_ptr<'a>(
        data: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(data.is_aligned(), "Misaligned pointer");
        debug_assert!(
            *size == 0 || *size == capacity,
            "Invalid size {size} for capacity {capacity}"
        );
        debug_assert!(
            capacity >= Data::SIZE,
            "Invalid capacity {capacity} for size {}",
            Data::SIZE
        );

        let size = NonNull::from_ref(size);

        DerefWrapper(MaybeData {
            data: *data,
            size,
            capacity,
        })
    }

    #[inline]
    #[track_caller]
    unsafe fn from_mut_ptr<'a>(
        data: &'a mut NonNull<Self::PointerType>,
        size: &'a mut *mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(!size.is_null(), "`null` pointer for non-`TrivialType` size");
        // SAFETY: Must be guaranteed by the caller + debug check above
        let size = unsafe { NonNull::new_unchecked(*size) };
        debug_assert!(data.is_aligned(), "Misaligned pointer");
        // SAFETY: Must be guaranteed by the caller
        {
            let size = unsafe { size.read() };
            debug_assert!(
                size == 0 || size == capacity,
                "Invalid size {size} for capacity {capacity}"
            );
        }
        debug_assert!(
            capacity >= Data::SIZE,
            "Invalid capacity {capacity} for size {}",
            Data::SIZE
        );

        DerefWrapper(MaybeData {
            data: *data,
            size,
            capacity,
        })
    }

    #[inline]
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>> {
        &self.data
    }

    #[inline]
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>> {
        &mut self.data
    }
}

impl<Data> IoTypeOptional for MaybeData<Data> where Data: TrivialType {}

impl<Data> MaybeData<Data>
where
    Data: TrivialType,
{
    /// Create a new shared instance from provided data reference.
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    pub const fn from_buffer(data: Option<&'_ Data>) -> impl Deref<Target = Self> + '_ {
        let (data, size) = if let Some(data) = data {
            (NonNull::from_ref(data), &Data::SIZE)
        } else {
            (NonNull::dangling(), &0)
        };

        DerefWrapper(Self {
            data,
            size: NonNull::from_ref(size),
            capacity: Data::SIZE,
        })
    }

    /// Create a new exclusive instance from provided data reference.
    ///
    /// `size` can be either `0` or `Data::SIZE`, indicating that value is missing or present
    /// accordingly.
    ///
    /// # Panics
    /// Panics if `size != 0 && size != Data::SIZE`
    //
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    #[track_caller]
    pub fn from_buffer_mut<'a>(
        buffer: &'a mut Data,
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(
            *size == 0 || *size == Data::SIZE,
            "Invalid size {size} (self size {})",
            Data::SIZE
        );

        DerefWrapper(Self {
            data: NonNull::from_mut(buffer),
            size: NonNull::from_ref(size),
            capacity: Data::SIZE,
        })
    }

    /// Create a new shared instance from provided memory buffer.
    ///
    /// `size` must be `0`.
    ///
    /// # Panics
    /// Panics if `size != 0`
    //
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    // TODO: Change `usize` to `u32` once stabilized `generic_const_exprs` feature allows us to do
    //  `CAPACITY as usize`
    #[track_caller]
    pub fn from_uninit<'a>(
        uninit: &'a mut MaybeUninit<Data>,
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert_eq!(*size, 0, "Invalid size");

        DerefWrapper(Self {
            data: NonNull::from_mut(uninit).cast::<Data>(),
            size: NonNull::from_mut(size),
            capacity: Data::SIZE,
        })
    }

    /// Try to get access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub const fn get(&self) -> Option<&Data> {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == self.capacity {
            // SAFETY: initialized
            Some(unsafe { self.data.as_ref() })
        } else {
            None
        }
    }

    /// Try to get exclusive access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut Data> {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == self.capacity {
            // SAFETY: initialized
            Some(unsafe { self.data.as_mut() })
        } else {
            None
        }
    }

    /// Initialize by inserting `Data` by value or replace existing value and return reference to it
    #[inline]
    pub fn replace(&mut self, data: Data) -> &mut Data {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(self.capacity);
        }
        // SAFETY: constructor guarantees that memory is aligned
        unsafe {
            self.data.write(data);
            self.data.as_mut()
        }
    }

    /// Remove `Data` inside and turn instance back into uninitialized
    #[inline]
    pub fn remove(&mut self) {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(0);
        }
    }

    /// Get exclusive access to initialized `Data`, running provided initialization function if
    /// necessary
    #[inline]
    pub fn get_mut_or_init_with<'a, Init>(&'a mut self, init: Init) -> &'a mut Data
    where
        Init: FnOnce(NonNull<Data>) -> &'a mut Data,
    {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == self.capacity {
            // SAFETY: initialized
            unsafe { self.data.as_mut() }
        } else {
            let data = init(self.data);
            // SAFETY: guaranteed to be initialized by constructors
            unsafe {
                self.size.write(self.capacity);
            }
            data
        }
    }

    /// Assume value is initialized
    ///
    /// # Safety
    /// Caller must ensure `Data` is actually properly initialized
    #[inline]
    pub unsafe fn assume_init(&mut self) -> &mut Data {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe {
            self.size.write(self.capacity);
        }
        // SAFETY: guaranteed to be initialized by caller, the rest of guarantees are provided by
        // constructors
        unsafe { self.data.as_mut() }
    }
}
