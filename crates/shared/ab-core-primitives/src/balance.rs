//! Balance-related primitives

use ab_io_type::metadata::IoTypeMetadataKind;
use ab_io_type::trivial_type::TrivialType;
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use core::{fmt, ptr};

/// Logically the same as `u128`, but aligned to `8` bytes instead of `16`.
///
/// Byte layout is the same as `u128`, just alignment is different
#[derive(Default, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct Balance(u64, u64);

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for Balance {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Balance as u8];
}

// Ensure this never mismatches with code in `ab-io-type` despite being in different crate
const {
    let (type_details, _metadata) = IoTypeMetadataKind::type_details(Balance::METADATA)
        .expect("Statically correct metadata; qed");
    assert!(size_of::<Balance>() == type_details.recommended_capacity as usize);
    assert!(align_of::<Balance>() == type_details.alignment.get() as usize);
}

impl fmt::Debug for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Balance").field(&self.as_u128()).finish()
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_u128().fmt(f)
    }
}

impl Ord for Balance {
    #[inline(always)]
    fn cmp(&self, other: &Balance) -> Ordering {
        self.as_u128().cmp(&other.as_u128())
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
        Self::new(self.as_u128().add(rhs.as_u128()))
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
        Self::new(self.as_u128().sub(rhs.as_u128()))
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
        Self::new(<u128 as Mul<Rhs>>::mul(self.as_u128(), rhs))
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
        Self::new(<u128 as Div<Rhs>>::div(self.as_u128(), rhs))
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

impl From<u128> for Balance {
    #[inline(always)]
    fn from(value: u128) -> Self {
        Self::new(value)
    }
}

impl From<Balance> for u128 {
    #[inline(always)]
    fn from(value: Balance) -> Self {
        value.as_u128()
    }
}

impl Balance {
    /// Minimum balance
    pub const MIN: Self = Self::new(0);
    /// Maximum balance
    pub const MAX: Self = Self::new(u128::MAX);

    /// Create a value from `u128`
    #[inline(always)]
    pub const fn new(n: u128) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe {
            result.as_mut_ptr().cast::<u128>().write_unaligned(n);
            result.assume_init()
        }
    }

    /// Turn value into `u128`
    #[inline(always)]
    pub const fn as_u128(self) -> u128 {
        // SAFETY: correct size, valid pointer, and all bits are valid
        unsafe { ptr::from_ref(&self).cast::<u128>().read_unaligned() }
    }
}
