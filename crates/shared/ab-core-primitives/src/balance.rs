//! Balance-related primitives

use ab_io_type::trivial_type::TrivialType;
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use core::{fmt, ptr};

/// Logically the same as `u128`, but aligned to `8` bytes instead of `16`.
///
/// Byte layout is the same as `u128`, just the alignment is different.
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct Balance(u64, u64);

impl fmt::Debug for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Balance").field(&u128::from(self)).finish()
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&u128::from(self), f)
    }
}

impl Ord for Balance {
    #[inline(always)]
    fn cmp(&self, other: &Balance) -> Ordering {
        u128::from(self).cmp(&u128::from(other))
    }
}

impl PartialOrd for Balance {
    #[inline(always)]
    fn partial_cmp(&self, other: &Balance) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Balance {
    type Output = Balance;

    #[inline(always)]
    #[track_caller]
    fn add(self, rhs: Balance) -> Balance {
        Self::from(u128::from(self).add(u128::from(rhs)))
    }
}

impl AddAssign for Balance {
    #[inline(always)]
    #[track_caller]
    fn add_assign(&mut self, rhs: Balance) {
        *self = *self + rhs;
    }
}

impl Sub for Balance {
    type Output = Balance;

    #[inline(always)]
    #[track_caller]
    fn sub(self, rhs: Balance) -> Balance {
        Self::from(u128::from(self).sub(u128::from(rhs)))
    }
}

impl SubAssign for Balance {
    #[inline(always)]
    #[track_caller]
    fn sub_assign(&mut self, rhs: Balance) {
        *self = *self - rhs;
    }
}

impl<Rhs> Mul<Rhs> for Balance
where
    u128: Mul<Rhs, Output = u128>,
{
    type Output = Balance;

    #[inline(always)]
    #[track_caller]
    fn mul(self, rhs: Rhs) -> Balance {
        Self::from(<u128 as Mul<Rhs>>::mul(u128::from(self), rhs))
    }
}

impl<Rhs> MulAssign<Rhs> for Balance
where
    u128: Mul<Rhs, Output = u128>,
{
    #[inline(always)]
    #[track_caller]
    fn mul_assign(&mut self, rhs: Rhs) {
        *self = *self * rhs;
    }
}

impl<Rhs> Div<Rhs> for Balance
where
    u128: Div<Rhs, Output = u128>,
{
    type Output = Balance;

    #[inline(always)]
    #[track_caller]
    fn div(self, rhs: Rhs) -> Balance {
        Self::from(<u128 as Div<Rhs>>::div(u128::from(self), rhs))
    }
}

impl<Rhs> DivAssign<Rhs> for Balance
where
    u128: Div<Rhs, Output = u128>,
{
    #[inline(always)]
    #[track_caller]
    fn div_assign(&mut self, rhs: Rhs) {
        *self = *self / rhs;
    }
}

const impl From<u128> for Balance {
    #[inline(always)]
    fn from(value: u128) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe {
            result.as_mut_ptr().cast::<u128>().write_unaligned(value);
            result.assume_init()
        }
    }
}

const impl From<&Balance> for u128 {
    #[inline(always)]
    fn from(value: &Balance) -> Self {
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe { ptr::from_ref(value).cast::<u128>().read_unaligned() }
    }
}

const impl From<Balance> for u128 {
    #[inline(always)]
    fn from(value: Balance) -> Self {
        Self::from(&value)
    }
}

impl Balance {
    /// Minimum balance
    pub const MIN: Self = Self::from(0);
    /// Maximum balance
    pub const MAX: Self = Self::from(u128::MAX);
}
