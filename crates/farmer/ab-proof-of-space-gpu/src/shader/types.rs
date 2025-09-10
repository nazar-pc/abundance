use crate::shader::num::{U128, U128T};
use core::iter::Step;
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
#[derive(Debug, Copy, Clone, Eq, PartialEq, From, Into)]
#[repr(C)]
pub struct Y(u32);

impl From<Y> for U128 {
    #[inline(always)]
    fn from(value: Y) -> Self {
        Self::from(value.0)
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into)]
#[repr(C)]
pub struct Position(u32);

impl From<Position> for usize {
    #[inline(always)]
    fn from(value: Position) -> Self {
        value.0 as Self
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
