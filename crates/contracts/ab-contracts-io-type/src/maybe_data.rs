use crate::trivial_type::TrivialType;
use crate::{IoType, IoTypeOptional};
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
/// it, which is simpler and sufficient in many cases.
pub struct MaybeData<Data>
where
    Data: TrivialType,
{
    data: NonNull<Data>,
    size: NonNull<u32>,
}

unsafe impl<Data> IoType for MaybeData<Data>
where
    Data: TrivialType,
{
    const CAPACITY: u32 = Data::CAPACITY;
    const METADATA: &[u8] = Data::METADATA;

    type PointerType = Data;

    #[inline]
    fn size(&self) -> u32 {
        // SAFETY: guaranteed to be initialized by constructors
        unsafe { self.size.read() }
    }

    unsafe fn set_size(&mut self, size: u32) {
        debug_assert!(
            size == 0 || size == Data::CAPACITY,
            "`set_size` called with invalid input"
        );

        // SAFETY: guaranteed to be initialized by constructors
        self.size.write(size);
    }

    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
    ) -> impl Deref<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned());
        debug_assert!(*size == 0 || *size == Self::CAPACITY);

        let data = ptr.cast::<Data>();
        // TODO: Use `NonNull::from_ref()` once stable
        let size = NonNull::from(size);

        MaybeDataWrapper(MaybeData { data, size })
    }

    unsafe fn from_ptr_mut<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
    ) -> impl DerefMut<Target = Self> + 'a {
        debug_assert!(ptr.is_aligned());
        debug_assert!(*size == 0 || *size == Self::CAPACITY);

        let data = ptr.cast::<Data>();
        // TODO: Use `NonNull::from_ref()` once stable
        let size = NonNull::from(size);

        MaybeDataWrapper(MaybeData { data, size })
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
    /// Try to get access to initialized `Data`, returns `None` if not initialized
    #[inline]
    pub fn get(&self) -> Option<&Data> {
        // SAFETY: guaranteed to be initialized by constructors
        if unsafe { self.size.read() } == Data::CAPACITY {
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
        if unsafe { self.size.read() } == Data::CAPACITY {
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
            self.size.write(Data::CAPACITY);
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
        if unsafe { self.size.read() } == Data::CAPACITY {
            // SAFETY: initialized
            unsafe { self.data.as_mut() }
        } else {
            let data = init(self.data);
            // SAFETY: guaranteed to be initialized by constructors
            unsafe {
                self.size.write(Data::CAPACITY);
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
        self.size.write(Data::CAPACITY);
        self.data.as_mut()
    }
}
