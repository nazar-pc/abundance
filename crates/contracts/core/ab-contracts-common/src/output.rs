//! Memory given to method calls as `#[output]`

use ab_io_type::trivial_type::AnyBitPattern;
use ab_io_type::{IoType, IoTypeMut};
use core::ptr::NonNull;

/// Memory that can be given to a method call as an `#[output]` of type `Target`.
///
/// A call can leave contents behind that are not a valid value of `Target`, for example when it
/// fails halfway, so the contents must be parsed before they are accessed, unless every bit
/// pattern is a valid value (see [`AnyBitPattern`]).
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
    label = "expected `&mut {Target}` (or a view of one) whose contents accept every bit pattern",
    note = "a failed call can leave invalid contents behind, so only memory whose contents accept \
    every bit pattern can be given to a call directly"
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
// (`IoType::as_mut_ptr()`). The value is returned unchanged by every call of `io_type_mut()` and is
// only accessible through `self` (`IoTypeMut` contract), so it isn't accessed while `self` is
// exclusively borrowed. Initialized contents are a valid value, since every bit pattern is.
unsafe impl<Arg> OutputArg<Arg::Target> for Arg
where
    Arg: IoTypeMut,
    <Arg::Target as IoType>::PointerType: AnyBitPattern,
{
    #[inline(always)]
    fn as_raw_output(&mut self) -> (NonNull<<Arg::Target as IoType>::PointerType>, u32, u32) {
        let value = self.io_type_mut();
        (
            IoType::as_mut_ptr(value),
            IoType::size(value),
            IoType::capacity(value),
        )
    }

    #[inline(always)]
    unsafe fn set_output_size(&mut self, size: u32) {
        // SAFETY: The size is valid and doesn't exceed the capacity, with that many bytes
        // initialized (function contract), which are a valid value, since every bit pattern is
        unsafe { IoType::set_size(self.io_type_mut(), size) }
    }
}
