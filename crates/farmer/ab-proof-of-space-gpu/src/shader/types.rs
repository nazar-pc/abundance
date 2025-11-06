#[cfg(all(test, not(target_arch = "spirv")))]
mod tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, MAX_TABLE_SIZE, NUM_BUCKETS, PARAM_BC, PARAM_EXT};
use crate::shader::num::{U128, U128T};
use core::iter::Step;
use core::mem::MaybeUninit;
use derive_more::{From, Into};

/// Stores data in lower bits
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into)]
#[repr(C)]
pub(super) struct X(u32);

impl Step for X {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        u32::steps_between(&start.0, &end.0)
    }

    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        u32::forward_checked(start.0, count).map(Self)
    }

    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        u32::backward_checked(start.0, count).map(Self)
    }
}

impl X {
    #[cfg(test)]
    pub(super) const ZERO: Self = Self(0);
}

/// Stores data in lower bits
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into)]
#[repr(C)]
pub struct Y(u32);

impl From<Y> for U128 {
    #[inline(always)]
    fn from(value: Y) -> Self {
        Self::from(value.0)
    }
}

impl Y {
    /// Get the first `K` bits
    #[inline(always)]
    pub(in super::super) const fn first_k_bits(self) -> u32 {
        self.0 >> PARAM_EXT
    }

    pub(super) fn into_bucket_index_and_r(self) -> (u32, R) {
        let bucket_index = self.0 / u32::from(PARAM_BC);
        let r = self.0 % u32::from(PARAM_BC);
        // SAFETY: `r` is within `0..PARAM_BC` range
        let r = unsafe { R::new(r) };
        (bucket_index, r)
    }
}

// TODO: The struct in this form currently doesn't compile:
//  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
// #[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into)]
// #[repr(C)]
// pub struct Position(u32);
//
// impl From<Position> for usize {
//     #[inline(always)]
//     fn from(value: Position) -> Self {
//         value.0 as Self
//     }
// }
//
// impl Position {
//     /// Position that can't exist
//     pub(super) const SENTINEL: Self = Self(u32::MAX >> (u32::BITS - MAX_TABLE_SIZE.bit_width()));
// }
//
// impl Position {
//     #[inline(always)]
//     pub(super) const fn uninit_array_from_repr_mut<const N: usize>(
//         array: &mut [MaybeUninit<u32>; N],
//     ) -> &mut [MaybeUninit<Self>; N] {
//         // SAFETY: `Position` is `#[repr(C)]` and guaranteed to have the same memory layout
//         unsafe { mem::transmute(array) }
//     }
// }

pub type Position = u32;

// TODO: Remove once normal `Position` struct can be used
pub(super) trait PositionExt: Sized {
    /// Position that can't exist
    const SENTINEL: Self;

    fn uninit_array_from_repr_mut<const N: usize>(
        array: &mut [MaybeUninit<u32>; N],
    ) -> &mut [MaybeUninit<Self>; N];

    // TODO: This is just `Position::from()` usually
    fn from_u32(value: u32) -> Self;
}

impl PositionExt for Position {
    const SENTINEL: Self = u32::MAX >> (u32::BITS - MAX_TABLE_SIZE.bit_width());

    #[inline(always)]
    fn uninit_array_from_repr_mut<const N: usize>(
        array: &mut [MaybeUninit<u32>; N],
    ) -> &mut [MaybeUninit<Self>; N] {
        array
    }

    #[inline(always)]
    fn from_u32(value: u32) -> Self {
        value
    }
}

/// Stores data in lower bits
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Metadata(U128);

impl Default for Metadata {
    #[inline(always)]
    fn default() -> Self {
        Self(U128::ZERO)
    }
}

impl From<Metadata> for U128 {
    #[inline(always)]
    fn from(value: Metadata) -> Self {
        value.0
    }
}

impl From<U128> for Metadata {
    #[inline(always)]
    fn from(value: U128) -> Self {
        Self(value)
    }
}

impl From<Position> for Metadata {
    #[inline(always)]
    fn from(value: Position) -> Self {
        Self(U128::from(value))
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
pub struct R(pub u32);

impl R {
    /// R that can't exist
    pub(super) const SENTINEL: Self = Self(u32::MAX);

    /// Create new `R` from provided value.
    ///
    /// # Safety
    /// `r` value must be within `0..PARAM_BC` range.
    #[inline(always)]
    pub(super) unsafe fn new(r: u32) -> Self {
        Self(r)
    }

    /// Similar to `new`, but also stores extra data alongside `r`.
    ///
    /// # Safety
    /// `r` value is expected to be within `0..PARAM_BC` range, `data` must contain at most
    /// `u32::BITS - (PARAM_BC - 1).bit_width()` bits of data in it.
    #[inline(always)]
    pub(super) unsafe fn new_with_data(r: u32, data: u32) -> Self {
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        Self(r | (data << const { (PARAM_BC - 1).bit_width() }))
    }

    /// Get the inner stored value, which in case of no extra data will be `r`, but may also include
    /// extra data if constructed with [`Self::new_with_data()`]. This is a more efficient
    /// alternative to [`Self::split()`] in case it is guaranteed that there is no `data` attached.
    #[inline(always)]
    pub(super) fn get_inner(&self) -> u32 {
        self.0
    }

    /// The inverse of [`Self::new_with_data()`], returns `(r, data)`
    #[inline(always)]
    pub(super) fn split(self) -> (u32, u32) {
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        (
            self.0 & (u32::MAX >> const { u32::BITS - (PARAM_BC - 1).bit_width() }),
            self.0 >> const { (PARAM_BC - 1).bit_width() },
        )
    }
}

/// A tuple of [`Position`] and [`Y`] with guaranteed memory layout
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
pub struct PositionR {
    /// Position
    pub position: Position,
    /// R
    pub r: R,
}

impl PositionR {
    /// PositionR that can't exist
    pub(super) const SENTINEL: Self = Self {
        position: Position::SENTINEL,
        r: R::SENTINEL,
    };
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Match {
    bucket_offset_right_position: u32,
}

impl Match {
    /// # Safety
    /// `bucket_offset` value must be within `0..MAX_BUCKET_SIZE` range, `left_position` and
    /// `right_position` must be within `0..(NUM_BUCKETS * MAX_BUCKET_SIZE)` range
    #[inline(always)]
    pub(super) unsafe fn new(bucket_offset: u32, right_position: Position) -> Self {
        const {
            assert!(
                (MAX_BUCKET_SIZE - 1).bit_width() + (NUM_BUCKETS * MAX_BUCKET_SIZE - 1).bit_width()
                    <= u32::BITS
            );
        }

        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        Self {
            bucket_offset_right_position: (bucket_offset
                << const { (NUM_BUCKETS * MAX_BUCKET_SIZE - 1).bit_width() })
                | right_position,
        }
    }

    #[inline(always)]
    pub fn bucket_offset(&self) -> u32 {
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        self.bucket_offset_right_position
            >> const { (NUM_BUCKETS * MAX_BUCKET_SIZE - 1).bit_width() }
    }

    #[inline(always)]
    pub fn right_position(&self) -> Position {
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        self.bucket_offset_right_position
            & (u32::MAX >> const { u32::BITS - (NUM_BUCKETS * MAX_BUCKET_SIZE - 1).bit_width() })
    }
}
