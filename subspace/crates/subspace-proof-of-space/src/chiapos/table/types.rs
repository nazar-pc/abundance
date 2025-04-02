use crate::chiapos::constants::PARAM_EXT;
use crate::chiapos::table::metadata_size_bytes;
use crate::chiapos::utils::EvaluatableUsize;
use core::iter::Step;
use core::mem;
use core::ops::Range;
use derive_more::{Add, AddAssign, From, Into};

/// Stores data in lower bits
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into, Add, AddAssign)]
#[repr(transparent)]
pub(in super::super) struct X(u32);

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

impl From<X> for u64 {
    #[inline(always)]
    fn from(value: X) -> Self {
        Self::from(value.0)
    }
}

impl From<X> for u128 {
    #[inline(always)]
    fn from(value: X) -> Self {
        Self::from(value.0)
    }
}

impl From<X> for usize {
    #[inline(always)]
    fn from(value: X) -> Self {
        value.0 as Self
    }
}

impl X {
    /// All possible values of `x` for given `K`
    pub(in super::super) const fn all<const K: u8>() -> Range<Self> {
        Self(0)..Self(1 << K)
    }
}

/// Stores data in lower bits
#[derive(Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into)]
#[repr(transparent)]
pub(in super::super) struct Y(u32);

impl From<Y> for u128 {
    #[inline(always)]
    fn from(value: Y) -> Self {
        Self::from(value.0)
    }
}

impl From<Y> for usize {
    #[inline(always)]
    fn from(value: Y) -> Self {
        value.0 as Self
    }
}

impl Y {
    pub(in super::super) const fn first_k_bits<const K: u8>(self) -> u32 {
        self.0 >> PARAM_EXT as usize
    }
}

#[derive(
    Debug, Default, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, From, Into, Add, AddAssign,
)]
#[repr(transparent)]
pub(in super::super) struct Position(u32);

impl Step for Position {
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

impl From<Position> for usize {
    #[inline(always)]
    fn from(value: Position) -> Self {
        value.0 as Self
    }
}

impl Position {
    pub(in super::super) const ZERO: Self = Self(0);
    pub(in super::super) const ONE: Self = Self(1);
}

/// Stores data in lower bits
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub(in super::super) struct Metadata<const K: u8, const TABLE_NUMBER: u8>(
    [u8; metadata_size_bytes(K, TABLE_NUMBER)],
)
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized;

impl<const K: u8, const TABLE_NUMBER: u8> Default for Metadata<K, TABLE_NUMBER>
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    #[inline(always)]
    fn default() -> Self {
        Self([0; metadata_size_bytes(K, TABLE_NUMBER)])
    }
}

impl<const K: u8, const TABLE_NUMBER: u8> From<Metadata<K, TABLE_NUMBER>> for u128
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    fn from(value: Metadata<K, TABLE_NUMBER>) -> Self {
        // `*_be_bytes()` is used such that `Ord`/`PartialOrd` impl works as expected
        let mut output = 0u128.to_be_bytes();
        output[mem::size_of::<u128>() - value.0.len()..].copy_from_slice(&value.0);

        Self::from_be_bytes(output)
    }
}

impl<const K: u8, const TABLE_NUMBER: u8> From<u128> for Metadata<K, TABLE_NUMBER>
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    /// If used incorrectly, will truncate information, it is up to implementation to ensure `u128`
    /// only contains data in lower bits and fits into internal byte array of `Metadata`
    fn from(value: u128) -> Self {
        Self(
            value.to_be_bytes()[mem::size_of::<u128>() - metadata_size_bytes(K, TABLE_NUMBER)..]
                .try_into()
                .expect("Size of internal byte array is always smaller or equal to u128; qed"),
        )
    }
}

impl<const K: u8, const TABLE_NUMBER: u8> From<X> for Metadata<K, TABLE_NUMBER>
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    #[inline(always)]
    fn from(value: X) -> Self {
        Self::from(u128::from(value))
    }
}
