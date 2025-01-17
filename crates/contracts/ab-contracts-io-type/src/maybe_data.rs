use crate::trivial_type::TrivialType;
use crate::{IoType, IoTypeOptional};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

struct MaybeDataWrapper<Data>(MaybeData<Data>)
where
    Data: TrivialType;

impl<Data> Deref for MaybeDataWrapper<Data>
where
    Data: TrivialType,
{
    type Target = MaybeData<Data>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Data> DerefMut for MaybeDataWrapper<Data>
where
    Data: TrivialType,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Wrapper type for `Data` that may or may not be filled with contents.
///
/// This is somewhat similar to [`VariableBytes`](crate::variable_bytes::VariableBytes), but instead
/// of variable size data structure allows to either have it or not have the contents or not have
/// it, which is simpler and more convenient API that is also sufficient in many cases.
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
    fn capacity(&self) -> u32 {
        self.size()
    }

    #[inline]
    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size == 0 || size == self.size(),
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
        debug_assert!(*size == 0 || *size == capacity, "Invalid size");
        debug_assert!(capacity as usize == size_of::<Data>(), "Invalid capacity");

        let data = ptr.cast::<Data>();
        // TODO: Use `NonNull::from_ref()` once stable
        let size = NonNull::from(size);

        MaybeDataWrapper(MaybeData {
            data,
            size,
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
        debug_assert!(*size == 0 || *size == capacity, "Invalid size");
        debug_assert!(capacity as usize == size_of::<Data>(), "Invalid capacity");

        let data = ptr.cast::<Data>();
        // TODO: Use `NonNull::from_ref()` once stable
        let size = NonNull::from(size);

        MaybeDataWrapper(MaybeData {
            data,
            size,
            capacity,
        })
    }
}

impl<Data> IoTypeOptional for MaybeData<Data>
where
    Data: TrivialType,
{
    #[inline]
    fn as_mut_ptr(&mut self) -> &mut NonNull<Self::PointerType> {
        &mut self.data
    }
}

impl<Data> MaybeData<Data>
where
    Data: TrivialType,
{
    /// Create a new shared instance from provided data reference.
    ///
    /// `size` can be either `0` or `size_of::<Data>()`, indicating that value is missing or present
    /// accordingly.
    ///
    /// # Panics
    /// Panics if `size != 0 && size != size_of::<Data>()`
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    pub fn from_buffer<'a>(data: &'a Data, size: &'a u32) -> impl Deref<Target = Self> + 'a {
        let capacity = size_of::<Data>() as u32;
        debug_assert!(*size == 0 || *size == capacity, "Invalid size");

        MaybeDataWrapper(Self {
            // TODO: Use `NonNull::from_ref()` once stable
            data: NonNull::from(data).cast::<<Self as IoType>::PointerType>(),
            // TODO: Use `NonNull::from_ref()` once stable
            size: NonNull::from(size),
            capacity,
        })
    }

    /// Create a new exclusive instance from provided data reference.
    ///
    /// `size` can be either `0` or `size_of::<Data>()`, indicating that value is missing or present
    /// accordingly.
    ///
    /// # Panics
    /// Panics if `size != 0 && size != size_of::<Data>()`
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes.
    pub fn from_buffer_mut<'a>(
        buffer: &'a mut Data,
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        let capacity = size_of::<Data>() as u32;
        debug_assert!(*size == 0 || *size == capacity, "Invalid size");

        MaybeDataWrapper(Self {
            // TODO: Use `NonNull::from_mut()` once stable
            data: NonNull::from(buffer).cast::<<Self as IoType>::PointerType>(),
            // TODO: Use `NonNull::from_mut()` once stable
            size: NonNull::from(size),
            capacity,
        })
    }

    /// Create a new shared instance from provided memory buffer.
    ///
    /// `size` must be `0`.
    ///
    /// # Panics
    /// Panics if `size != 0`
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes.
    // TODO: Change `usize` to `u32` once stabilized `generic_const_exprs` feature allows us to do
    //  `CAPACITY as usize`
    pub fn from_uninit<'a>(
        uninit: &'a mut MaybeUninit<Data>,
        size: &'a mut u32,
    ) -> impl Deref<Target = Self> + 'a {
        let capacity = size_of::<Data>() as u32;
        debug_assert!(*size == 0, "Invalid size");

        MaybeDataWrapper(Self {
            // TODO: Use `NonNull::from_ref()` once stable
            data: NonNull::from(uninit).cast::<<Self as IoType>::PointerType>(),
            // TODO: Use `NonNull::from_mut()` once stable
            size: NonNull::from(size),
            capacity,
        })
    }

    /// Try to get access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub fn get(&self) -> Option<&Data> {
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
