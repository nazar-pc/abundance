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
//     pub(super) const ZERO: Self = Self(0);
//     /// Position that can't exist
//     pub(super) const SENTINEL: Self = Self(u32::MAX);
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
    const ZERO: Self;
    /// Position that can't exist
    const SENTINEL: Self;

    fn uninit_array_from_repr_mut<const N: usize>(
        array: &mut [MaybeUninit<u32>; N],
    ) -> &mut [MaybeUninit<Self>; N];

    // TODO: This is just `Position::from()` usually
    fn from_u32(value: u32) -> Self;
}

impl PositionExt for Position {
    const ZERO: Self = 0;
    const SENTINEL: Self = u32::MAX;

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

/// A tuple of [`Position`] and [`Y`] with guaranteed memory layout
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
pub struct PositionY {
    /// Position
    pub position: Position,
    /// Y
    pub y: Y,
}
