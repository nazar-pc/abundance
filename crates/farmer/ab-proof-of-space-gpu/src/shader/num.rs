#[cfg(test)]
mod tests;

use core::cmp::{Eq, PartialEq};
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Shl, ShlAssign,
    Shr, ShrAssign, Sub, SubAssign,
};

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
/// `u64` polyfill for SPIR-V, has the same in-memory representation as `u64` on little-endian
/// platform
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct U64(pub [u32; 2]);

impl From<u32> for U64 {
    fn from(n: u32) -> Self {
        Self::from_u32(n)
    }
}

impl U64 {
    const ZERO: Self = Self([0; _]);
    pub const BITS: u32 = u64::BITS;

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub fn to_be_bytes(self) -> [u8; 8] {
        let high = self.0[1].to_be_bytes();
        let low = self.0[0].to_be_bytes();

        [
            high[0], high[1], high[2], high[3], low[0], low[1], low[2], low[3],
        ]
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub fn from_be_bytes(bytes: [u8; 8]) -> Self {
        let high = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let low = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

        Self([low, high])
    }

    pub const fn from_u32(n: u32) -> Self {
        Self([n, 0])
    }

    pub fn as_u32(&self) -> u32 {
        self.0[0]
    }
}

impl Add for U64 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let (res, overflow) = self.0[0].overflowing_add(other.0[0]);

        Self([res, self.0[1] + other.0[1] + overflow as u32])
    }
}

impl AddAssign for U64 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for U64 {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        let (res, overflow) = self.0[0].overflowing_sub(other.0[0]);

        Self([res, self.0[1] - other.0[1] - overflow as u32])
    }
}

impl SubAssign for U64 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl BitAnd for U64 {
    type Output = Self;

    #[inline]
    fn bitand(self, other: Self) -> Self {
        Self([self.0[0] & other.0[0], self.0[1] & other.0[1]])
    }
}

impl BitAndAssign for U64 {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitXor for U64 {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        Self([self.0[0] ^ other.0[0], self.0[1] ^ other.0[1]])
    }
}

impl BitXorAssign for U64 {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl BitOr for U64 {
    type Output = Self;

    #[inline]
    fn bitor(self, other: Self) -> Self {
        Self([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }
}

impl BitOrAssign for U64 {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl Shl<u32> for U64 {
    type Output = Self;

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
    fn shl_assign(&mut self, shift: u32) {
        *self = *self << shift;
    }
}

impl Shr<u32> for U64 {
    type Output = Self;

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
    fn shr_assign(&mut self, shift: u32) {
        *self = *self >> shift;
    }
}

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
/// `u128` polyfill for SPIR-V has the same in-memory representation as `u64` on little-endian
// /// platform
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct U128(pub [U64; 2]);

impl U128 {
    #[expect(dead_code, reason = "Not used yet")]
    const ZERO: Self = Self([U64::ZERO; _]);
    #[expect(dead_code, reason = "Not used yet")]
    pub const BITS: u32 = u64::BITS;

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub fn to_be_bytes(self) -> [u8; 16] {
        let low = &self.0[0];
        let high = &self.0[1];

        let high0 = high.0[1].to_be_bytes();
        let high1 = high.0[0].to_be_bytes();
        let low0 = low.0[1].to_be_bytes();
        let low1 = low.0[0].to_be_bytes();

        [
            high0[0], high0[1], high0[2], high0[3], high1[0], high1[1], high1[2], high1[3],
            low0[0], low0[1], low0[2], low0[3], low1[0], low1[1], low1[2], low1[3],
        ]
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        let high0 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let high1 = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let low0 = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let low1 = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);

        Self([U64([low1, low0]), U64([high1, high0])])
    }
}

impl Add for U128 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        // Add lower 64 bits
        let (low_sum0, carry0) = self.0[0].0[0].overflowing_add(other.0[0].0[0]);
        let (low_sum1, carry1a) = self.0[0].0[1].overflowing_add(other.0[0].0[1]);
        let (low_sum1, carry1b) = low_sum1.overflowing_add(carry0 as u32);
        let carry_low = carry1a || carry1b;

        let low = U64([low_sum0, low_sum1]);

        // Add upper 64 bits with carry
        let (high_sum0, _) = self.0[1].0[0].overflowing_add(other.0[1].0[0]);
        let (high_sum1, _) = self.0[1].0[1].overflowing_add(other.0[1].0[1]);
        let (high_sum0, _) = high_sum0.overflowing_add(carry_low as u32);

        let high = U64([high_sum0, high_sum1]);

        Self([low, high])
    }
}

impl AddAssign for U128 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl Sub for U128 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        // Subtract lower 64 bits
        let (low_diff0, borrow0) = self.0[0].0[0].overflowing_sub(other.0[0].0[0]);
        let (low_diff1, borrow1a) = self.0[0].0[1].overflowing_sub(other.0[0].0[1]);
        let (low_diff1, borrow1b) = low_diff1.overflowing_sub(borrow0 as u32);
        let borrow_low = borrow1a || borrow1b;

        let low = U64([low_diff0, low_diff1]);

        // Subtract upper 64 bits with borrow
        let (high_diff0, _) = self.0[1].0[0].overflowing_sub(other.0[1].0[0]);
        let (high_diff1, _) = self.0[1].0[1].overflowing_sub(other.0[1].0[1]);
        let (high_diff0, _) = high_diff0.overflowing_sub(borrow_low as u32);

        let high = U64([high_diff0, high_diff1]);

        Self([low, high])
    }
}

impl SubAssign for U128 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl BitAnd for U128 {
    type Output = Self;

    #[inline]
    fn bitand(self, other: U128) -> U128 {
        let Self(arr1) = self;
        let Self(arr2) = other;
        let mut ret = [U64::ZERO; 2];
        for i in 0..2 {
            ret[i] = arr1[i] & arr2[i];
        }
        Self(ret)
    }
}

impl BitAndAssign for U128 {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl BitXor for U128 {
    type Output = Self;

    #[inline]
    fn bitxor(self, other: Self) -> Self {
        let Self(arr1) = self;
        let Self(arr2) = other;
        let mut ret = [U64::ZERO; 2];
        for i in 0..2 {
            ret[i] = arr1[i] ^ arr2[i];
        }
        Self(ret)
    }
}

impl BitXorAssign for U128 {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl BitOr for U128 {
    type Output = Self;

    #[inline]
    fn bitor(self, other: Self) -> Self {
        let Self(arr1) = self;
        let Self(arr2) = other;
        let mut ret = [U64::ZERO; 2];
        for i in 0..2 {
            ret[i] = arr1[i] | arr2[i];
        }
        Self(ret)
    }
}

impl BitOrAssign for U128 {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl Shl<u32> for U128 {
    type Output = Self;

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
    fn shl_assign(&mut self, shift: u32) {
        *self = *self << shift;
    }
}

impl Shr<u32> for U128 {
    type Output = Self;

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
    fn shr_assign(&mut self, shift: u32) {
        *self = *self >> shift;
    }
}
