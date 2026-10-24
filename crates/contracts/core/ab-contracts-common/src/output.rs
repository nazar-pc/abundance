//! Memory given to method calls as `#[output]`

use crate::ContractError;
use ab_io_type::IoType;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};
use ab_io_type::variable_elements::VariableElements;
use core::fmt;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::DerefMut;
use core::ptr::NonNull;

/// Memory that can be given to a method call as an `#[output]` of type `Target`.
///
/// A call can leave contents behind that are not a valid value of `Target`, for example when it
/// fails halfway, so the contents must be parsed before they are accessed, unless every bit
/// pattern is a valid value (see [`AnyBitPattern`]). [`Lent`] does this for memory whose contents
/// need parsing.
///
/// A call gets the pointer, size and capacity from [`Self::as_raw_output()`]. Until `self` is used
/// again, the call may write initialized bytes anywhere within the capacity, even if it fails. If
/// it succeeds, the size it reports is set with [`Self::set_output_size()`].
///
/// # Safety
/// [`Self::as_raw_output()`] returns a pointer aligned for `Target::PointerType`, and a size that
/// is valid for `Target` and doesn't exceed the capacity it returns. The pointer is valid for
/// writes of the capacity, but for reads only of the size, with that many bytes initialized: the
/// bytes after them may be uninitialized and must not be read before they are written. While
/// `self` stays exclusively borrowed after that, the memory is not accessed other than through the
/// pointer. Whatever initialized bytes a call leaves within the size (the initial one or the last
/// one set with [`Self::set_output_size()`]) are never accessed as `Target` without being parsed
/// first, unless every bit pattern is a valid value of `Target::PointerType`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be given to a call as an `#[output]` of type `{Target}`",
    label = "expected `&mut {Target}` whose contents accept every bit pattern, or \
    `&mut Lent<{Target}>`",
    note = "a failed call can leave invalid contents behind, use `Lent::new()` for a local value, \
    `Lent::<MaybeData<'_, _>>::from_uninit()` or \
    `Lent::<VariableElements<'_, _>>::from_uninit()` for a local container, or \
    `Env::lend_output()` for an `#[output]` the method received"
)]
pub unsafe trait OutputArg<Target>
where
    Target: IoType,
{
    /// Pointer to the contents, size and capacity to give to a call
    fn as_raw_output(&mut self) -> (NonNull<Target::PointerType>, u32, u32);

    /// Set the size that a successful call reported.
    ///
    /// # Safety
    /// `size` must be valid for `Target` (see [`IoType::try_from_ptr()`]) and not exceed the
    /// capacity, and that many bytes must be initialized.
    unsafe fn set_output_size(&mut self, size: u32);
}

// Without this attribute rustc reports the unsatisfied `AnyBitPattern` bound of the contents of
// containers instead of the message of `OutputArg`
#[diagnostic::do_not_recommend]
// SAFETY: Pointer, size and capacity are those of a valid value, the pointer can be used for reads
// of the size and writes of the capacity while the value isn't accessed otherwise
// (`IoType::as_mut_ptr()`), which it isn't while `self` is exclusively borrowed. Initialized
// contents are a valid value, since every bit pattern is.
unsafe impl<T> OutputArg<T> for T
where
    T: IoType,
    T::PointerType: AnyBitPattern,
{
    #[inline(always)]
    fn as_raw_output(&mut self) -> (NonNull<T::PointerType>, u32, u32) {
        (
            IoType::as_mut_ptr(self),
            IoType::size(self),
            IoType::capacity(self),
        )
    }

    #[inline(always)]
    unsafe fn set_output_size(&mut self, size: u32) {
        // SAFETY: The size is valid and doesn't exceed the capacity, with that many bytes
        // initialized (function contract), which are a valid value, since every bit pattern is
        unsafe { IoType::set_size(self, size) }
    }
}

enum LentValue<'a, T> {
    /// A plain value owned by `Lent`, which may not be a valid value after a call
    Owned(MaybeUninit<T>),
    /// A container view owned by `Lent` over memory borrowed for `'a`. The view itself is valid,
    /// while its contents may not be after a call.
    View(T),
    /// A plain value or a container view borrowed for `'a`
    Borrowed(NonNull<T>, PhantomData<&'a mut T>),
}

/// Memory lent to calls as an `#[output]`, whose contents are parsed before they are accessed.
///
/// A call can leave contents behind that are not a valid value, for example when it fails halfway,
/// so the contents are only accessible through [`Self::get_mut()`], which parses them once after
/// each call. Plain values can also be replaced with [`Self::set()`]. Memory whose contents accept
/// every bit pattern can be given to calls directly, see [`OutputArg`].
///
/// Created with [`Self::new()`] for a local value, with `Lent::<MaybeData<'_, _>>::from_uninit()`
/// or `Lent::<VariableElements<'_, _>>::from_uninit()` for a local container, and with
/// [`Env::lend_output()`] for an `#[output]` the method received.
///
/// [`Env::lend_output()`]: crate::env::Env::lend_output
pub struct Lent<'a, T>
where
    T: IoType,
{
    value: LentValue<'a, T>,
    /// Pointer to the contents, size and capacity, read from the value again while it is
    /// `checked`
    ptr: NonNull<T::PointerType>,
    size: u32,
    capacity: u32,
    /// Neither a call nor a size set changed the contents since they were last known to be valid
    checked: bool,
}

impl<T> fmt::Debug for Lent<'_, T>
where
    T: IoType,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Contents may not be a valid value
        f.debug_struct("Lent")
            .field("size", &self.size)
            .field("capacity", &self.capacity)
            .field("checked", &self.checked)
            .finish_non_exhaustive()
    }
}

// SAFETY: Pointer, size and capacity are those of the value when it was last known to be valid
// (created, parsed or set), so the pointer can be used for reads of the size and writes of the
// capacity (`IoType::as_mut_ptr()`). Since then, only calls changed the contents, by writing
// initialized bytes within the capacity, and the size, through `Self::set_output_size()`. The value
// is owned by `self` or borrowed by it for `'a`, so it isn't accessed while `self` is exclusively
// borrowed. The contents are only accessed as `T` after `Self::get_mut()` parsed them or
// `Self::set()` wrote a valid value.
unsafe impl<T> OutputArg<T> for Lent<'_, T>
where
    T: IoType,
{
    #[inline(always)]
    fn as_raw_output(&mut self) -> (NonNull<T::PointerType>, u32, u32) {
        let raw_parts = self.raw_parts();
        self.checked = false;
        raw_parts
    }

    #[inline(always)]
    unsafe fn set_output_size(&mut self, size: u32) {
        // Make sure the cached size is that of the value
        self.raw_parts();
        // The only valid size of a plain value is its size, so a different size means `T` is a
        // container view, which is a valid value regardless of its contents
        if size != self.size {
            // SAFETY: A container view, which is valid, owned by `self` or borrowed by it, and not
            // referenced elsewhere while `self` is exclusively borrowed
            let value = unsafe { self.value_ptr().as_mut() };
            // SAFETY: The size is valid and doesn't exceed the capacity, with that many bytes
            // initialized (function contract), the contents are only accessed after
            // `Self::get_mut()` parsed them
            unsafe {
                IoType::set_size(value, size);
            }
            self.size = size;
        }
        // The contents may have changed together with the size
        self.checked = false;
    }
}

impl<T> Lent<'static, T>
where
    T: TrivialType,
{
    /// Lend a local value
    #[inline(always)]
    pub fn new(value: T) -> Self {
        Self {
            value: LentValue::Owned(MaybeUninit::new(value)),
            // Read from the value before it is given to a call
            ptr: NonNull::dangling(),
            size: T::SIZE,
            capacity: T::SIZE,
            checked: true,
        }
    }
}

impl<'a, Data> Lent<'a, MaybeData<'a, Data>>
where
    Data: TrivialType,
    MaybeData<'a, Data>: IoType,
{
    /// Lend a local [`MaybeData`] over uninitialized memory, like [`MaybeData::from_uninit()`].
    ///
    /// Returns `None` if `size != 0`.
    #[inline(always)]
    pub fn from_uninit(uninit: &'a mut MaybeUninit<Data>, size: &'a mut u32) -> Option<Self> {
        Some(Self::from_view(MaybeData::from_uninit(uninit, size)?))
    }
}

impl<'a, Element, const RECOMMENDED_ALLOCATION: u32>
    Lent<'a, VariableElements<'a, Element, RECOMMENDED_ALLOCATION>>
where
    Element: TrivialType,
    VariableElements<'a, Element, RECOMMENDED_ALLOCATION>: IoType,
{
    /// Lend a local [`VariableElements`] over uninitialized memory, like
    /// [`VariableElements::from_uninit()`].
    ///
    /// Returns `None` if `size != 0` or if the buffer is larger than `u32::MAX` bytes.
    #[inline(always)]
    pub fn from_uninit(uninit: &'a mut [MaybeUninit<Element>], size: &'a mut u32) -> Option<Self> {
        Some(Self::from_view(VariableElements::from_uninit(
            uninit, size,
        )?))
    }
}

impl<'a, T> Lent<'a, T>
where
    T: IoType,
{
    /// Lend a container view over memory borrowed for `'a`
    #[inline(always)]
    fn from_view(mut view: T) -> Self {
        let ptr = IoType::as_mut_ptr(&mut view);
        let size = IoType::size(&view);
        let capacity = IoType::capacity(&view);
        Self {
            value: LentValue::View(view),
            ptr,
            size,
            capacity,
            checked: true,
        }
    }

    /// Lend `value`, which is only borrowed for `'a`, rather than until the method returns like
    /// with [`Env::lend_output()`].
    ///
    /// # Safety
    /// After the returned `Lent` was given to a call, `value` may hold contents that are not a
    /// valid value. Once the `Lent` is no longer used, the contents of `value` must not be accessed
    /// without being parsed first, unless the last use of the `Lent` after its last call was a
    /// successful [`Self::get_mut()`] or [`Self::set()`], or the contents of `T` accept every bit
    /// pattern.
    ///
    /// [`Env::lend_output()`]: crate::env::Env::lend_output
    #[inline(always)]
    pub unsafe fn from_mut_unchecked(value: &'a mut T) -> Self {
        let ptr = IoType::as_mut_ptr(value);
        let size = IoType::size(value);
        let capacity = IoType::capacity(value);
        Self {
            value: LentValue::Borrowed(NonNull::from_mut(value), PhantomData),
            ptr,
            size,
            capacity,
            checked: true,
        }
    }

    /// Exclusive access to the contents, which are parsed once after each call.
    ///
    /// Repeated calls without a call in between don't parse the contents again. Returns
    /// [`ContractError::BadOutput`] if they are not a valid value of `T`. They then stay
    /// inaccessible until [`Self::set()`], which only exists for plain values, replaces them.
    /// Lending them again doesn't help either: methods generated by `#[contract]` reject outputs
    /// with invalid initial contents with [`ContractError::BadInput`].
    ///
    /// A container is returned as a new view of the lent memory that only lives while `self` is
    /// borrowed (see [`IoType::reborrow_mut()`]). Code that replaces it, for example with a view
    /// of other memory, doesn't change what `self` lends to calls.
    #[inline(always)]
    pub fn get_mut(
        &mut self,
    ) -> Result<impl DerefMut<Target = T::WithLifetime<'_>> + '_, ContractError> {
        if !self.checked {
            let (mut ptr, mut size, capacity) = self.raw_parts();
            // SAFETY: The pointer is aligned and valid for writes of the capacity and for reads of
            // the size, which is valid for `T` and doesn't exceed the capacity, with that many
            // bytes initialized. The value was valid with these parts, and calls since then only
            // wrote initialized bytes within the capacity and set a valid size
            // (`OutputArg::set_output_size()` contract). Nothing else accesses the memory while
            // `self` is exclusively borrowed, and nothing is moved out of the returned value,
            // which is dropped right away.
            if unsafe { T::try_from_mut_ptr(&mut ptr, &mut size, capacity) }.is_none() {
                return Err(ContractError::BadOutput);
            }
            self.checked = true;
        }
        // SAFETY: A valid value, since it was parsed above or neither a call nor a size set changed
        // it since it was last known to be valid, owned by `self` or borrowed by it, and not
        // referenced elsewhere while `self` is borrowed
        let value = unsafe { self.value_ptr().as_mut() };
        Ok(value.reborrow_mut())
    }

    /// Pointer to the value, which may not be valid unless `checked`
    #[inline(always)]
    fn value_ptr(&mut self) -> NonNull<T> {
        match &mut self.value {
            LentValue::Owned(value) => NonNull::from_mut(value).cast::<T>(),
            LentValue::View(view) => NonNull::from_mut(view),
            LentValue::Borrowed(value, _) => *value,
        }
    }

    /// Pointer to the contents, size and capacity, read from the value unless a call or a size set
    /// may have changed it since it was last known to be valid
    #[inline(always)]
    fn raw_parts(&mut self) -> (NonNull<T::PointerType>, u32, u32) {
        if self.checked {
            // SAFETY: A valid value, since neither a call nor a size set changed it since it was
            // created, parsed or set, owned by `self` or borrowed by it, and not referenced
            // elsewhere while `self` is exclusively borrowed
            let value = unsafe { self.value_ptr().as_mut() };
            self.ptr = IoType::as_mut_ptr(value);
            self.size = IoType::size(value);
            self.capacity = IoType::capacity(value);
        } else if let LentValue::Owned(value) = &mut self.value {
            // An owned value moves together with `Lent`, unlike the memory of other values
            self.ptr = NonNull::from_mut(value).cast::<T::PointerType>();
        }
        (self.ptr, self.size, self.capacity)
    }
}

impl<T> Lent<'_, T>
where
    T: TrivialType,
{
    /// Replace the contents with a valid value
    #[inline(always)]
    pub fn set(&mut self, value: T) -> &mut T {
        let mut ptr = self.value_ptr();
        // SAFETY: Aligned and valid for writes of `T`, owned by `self` or borrowed by it, and not
        // referenced elsewhere while `self` is exclusively borrowed
        unsafe {
            ptr.write(value);
        }
        self.checked = true;
        // SAFETY: A valid value was written above, not referenced elsewhere while `self` is
        // borrowed
        unsafe { ptr.as_mut() }
    }
}
