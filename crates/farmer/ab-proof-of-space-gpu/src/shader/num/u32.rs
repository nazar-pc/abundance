#[cfg(test)]
mod tests;

use crate::shader::num::{U64T, U128T};
use core::cmp::{Eq, PartialEq};
use core::mem;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Shl, ShlAssign,
    Shr, ShrAssign, Sub, SubAssign,
};

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
/// `u64` polyfill for SPIR-V, has the same in-memory representation as `u64` on little-endian
/// platform
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub(in super::super) struct U64([u32; 2]);

impl From<u32> for U64 {
    #[inline(always)]
    fn from(n: u32) -> Self {
        Self([n, 0])
    }
}

impl U64 {
    const ZERO: Self = Self([0; _]);
}

impl U64T for U64 {
    #[inline(always)]
    fn from_lo_hi(lo: u32, hi: u32) -> Self {
        Self([lo, hi])
    }

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; 8] {
        let low = self.0[0].to_be_bytes();
        let high = self.0[1].to_be_bytes();

        [
            high[0], high[1], high[2], high[3], low[0], low[1], low[2], low[3],
        ]
    }

    #[inline(always)]
    fn from_be_bytes(bytes: [u8; 8]) -> Self {
        let low = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let high = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        Self([low, high])
    }

    #[inline(always)]
    fn as_u32(self) -> u32 {
        self.0[0]
    }
}

impl Add for U64 {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: Self) -> Self {
        let (lo, carry) = self.0[0].carrying_add(other.0[0], false);
        let (hi, _) = self.0[1].carrying_add(other.0[1], carry);

        Self([lo, hi])
    }
}

impl AddAssign for U64 {
    #[inline(always)]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for U64 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, other: Self) -> Self {
        let (lo, borrow) = self.0[0].borrowing_sub(other.0[0], false);
        let (hi, _) = self.0[1].borrowing_sub(other.0[1], borrow);

        Self([lo, hi])
    }
}

impl SubAssign for U64 {
    #[inline(always)]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl BitAnd for U64 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, other: Self) -> Self {
        Self([self.0[0] & other.0[0], self.0[1] & other.0[1]])
    }
}

impl BitAndAssign for U64 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitXor for U64 {
    type Output = Self;

    #[inline(always)]
    fn bitxor(self, other: Self) -> Self {
        Self([self.0[0] ^ other.0[0], self.0[1] ^ other.0[1]])
    }
}

impl BitXorAssign for U64 {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl BitOr for U64 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, other: Self) -> Self {
        Self([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }
}

impl BitOrAssign for U64 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl Shl<u32> for U64 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, shift: u32) -> Self {
        if shift == 0 {
            return self;
        }

        let [low, high] = self.0;

        if shift < u32::BITS {
            let new_high = (high << shift) | (low >> (u32::BITS - shift));
            let new_low = low << shift;
            Self([new_low, new_high])
        } else {
            let new_high = low << (shift - u32::BITS);
            Self([0, new_high])
        }
    }
}

impl ShlAssign<u32> for U64 {
    #[inline(always)]
    fn shl_assign(&mut self, shift: u32) {
        *self = *self << shift;
    }
}

impl Shr<u32> for U64 {
    type Output = Self;

    #[inline(always)]
    fn shr(self, shift: u32) -> Self {
        if shift == 0 {
            return self;
        }

        let [low, high] = self.0;

        if shift < u32::BITS {
            let new_low = (low >> shift) | (high << (u32::BITS - shift));
            let new_high = high >> shift;
            Self([new_low, new_high])
        } else {
            let new_low = high >> (shift - u32::BITS);
            Self([new_low, 0])
        }
    }
}

impl ShrAssign<u32> for U64 {
    #[inline(always)]
    fn shr_assign(&mut self, shift: u32) {
        *self = *self >> shift;
    }
}

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
/// `u128` polyfill for SPIR-V has the same in-memory representation as `u64` on little-endian
// /// platform
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub(in super::super) struct U128([U64; 2]);

impl From<u32> for U128 {
    #[inline(always)]
    fn from(n: u32) -> Self {
        Self([U64::from(n), U64::ZERO])
    }
}

impl U128T for U128 {
    const ZERO: Self = Self([U64::ZERO; 2]);

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; 16] {
        let low = &self.0[0];
        let high = &self.0[1];

        let low0 = low.0[1].to_be_bytes();
        let low1 = low.0[0].to_be_bytes();
        let high0 = high.0[1].to_be_bytes();
        let high1 = high.0[0].to_be_bytes();

        [
            high0[0], high0[1], high0[2], high0[3], high1[0], high1[1], high1[2], high1[3],
            low0[0], low0[1], low0[2], low0[3], low1[0], low1[1], low1[2], low1[3],
        ]
    }

    #[inline(always)]
    fn from_be_bytes(bytes: [u8; 16]) -> Self {
        let low0 = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let low1 = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let high0 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let high1 = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Self([U64([low1, low0]), U64([high1, high0])])
    }

    #[inline(always)]
    fn as_be_bytes_to_le_u32_words(&self) -> [u32; 4] {
        // SAFETY: All bit patterns are valid, alignment is the same
        let be_words = unsafe { mem::transmute::<&[U64; 2], &[u32; 4]>(&self.0) };

        [
            be_words[3].swap_bytes(),
            be_words[2].swap_bytes(),
            be_words[1].swap_bytes(),
            be_words[0].swap_bytes(),
        ]
    }

    #[inline(always)]
    fn from_le_u32_words_as_be_bytes(words: &[u32; 4]) -> Self {
        let be_words = [
            words[3].swap_bytes(),
            words[2].swap_bytes(),
            words[1].swap_bytes(),
            words[0].swap_bytes(),
        ];

        // SAFETY: All bit patterns are valid, alignment is the same
        Self(unsafe { mem::transmute::<[u32; 4], [U64; 2]>(be_words) })
    }
}

impl Add for U128 {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: Self) -> Self {
        let (lo0, c0) = self.0[0].0[0].carrying_add(other.0[0].0[0], false);
        let (lo1, c1) = self.0[0].0[1].carrying_add(other.0[0].0[1], c0);
        let (hi0, c2) = self.0[1].0[0].carrying_add(other.0[1].0[0], c1);
        let (hi1, _) = self.0[1].0[1].carrying_add(other.0[1].0[1], c2);

        Self([U64([lo0, lo1]), U64([hi0, hi1])])
    }
}

impl AddAssign for U128 {
    #[inline(always)]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for U128 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, other: Self) -> Self {
        let (lo0, b0) = self.0[0].0[0].borrowing_sub(other.0[0].0[0], false);
        let (lo1, b1) = self.0[0].0[1].borrowing_sub(other.0[0].0[1], b0);
        let (hi0, b2) = self.0[1].0[0].borrowing_sub(other.0[1].0[0], b1);
        let (hi1, _) = self.0[1].0[1].borrowing_sub(other.0[1].0[1], b2);

        Self([U64([lo0, lo1]), U64([hi0, hi1])])
    }
}

impl SubAssign for U128 {
    #[inline(always)]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl BitAnd for U128 {
    type Output = Self;

    #[inline(always)]
    fn bitand(self, other: U128) -> U128 {
        Self([self.0[0] & other.0[0], self.0[1] & other.0[1]])
    }
}

impl BitAndAssign for U128 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitXor for U128 {
    type Output = Self;

    #[inline(always)]
    fn bitxor(self, other: Self) -> Self {
        Self([self.0[0] ^ other.0[0], self.0[1] ^ other.0[1]])
    }
}

impl BitXorAssign for U128 {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl BitOr for U128 {
    type Output = Self;

    #[inline(always)]
    fn bitor(self, other: Self) -> Self {
        Self([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }
}

impl BitOrAssign for U128 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl Shl<u32> for U128 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, shift: u32) -> Self {
        if shift == 0 {
            return self;
        }

        let low = self.0[0];
        let high = self.0[1];

        if shift < u64::BITS {
            let low_shifted = low << shift;
            let high_shifted = high << shift;

            let carry = low >> (u64::BITS - shift);
            let new_high = U64([
                high_shifted.0[0] | carry.0[0],
                high_shifted.0[1] | carry.0[1],
            ]);

            Self([low_shifted, new_high])
        } else {
            let new_low = U64([0, 0]);
            let shifted = low << (shift - u64::BITS);
            Self([new_low, shifted])
        }
    }
}

impl ShlAssign<u32> for U128 {
    #[inline(always)]
    fn shl_assign(&mut self, shift: u32) {
        *self = *self << shift;
    }
}

impl Shr<u32> for U128 {
    type Output = Self;

    #[inline(always)]
    fn shr(self, shift: u32) -> Self {
        if shift == 0 {
            return self;
        }

        let low = self.0[0];
        let high = self.0[1];

        if shift < u64::BITS {
            let low_shifted = low >> shift;
            let high_shifted = high >> shift;

            let carry = high << (u64::BITS - shift);
            let new_low = U64([low_shifted.0[0] | carry.0[0], low_shifted.0[1] | carry.0[1]]);

            Self([new_low, high_shifted])
        } else {
            let shifted = high >> (shift - u64::BITS);
            Self([shifted, U64([0, 0])])
        }
    }
}

impl ShrAssign<u32> for U128 {
    #[inline(always)]
    fn shr_assign(&mut self, shift: u32) {
        *self = *self >> shift;
    }
}
