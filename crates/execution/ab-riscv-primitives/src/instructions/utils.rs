//! Utility types

#[cfg(test)]
mod tests;

use core::fmt;
use core::ops::{Shl, Shr};

/// New type for unsigned integers that stores 24-bit numbers
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct U24([u8; 3]);

impl fmt::Debug for U24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.to_u32(), f)
    }
}

impl fmt::Display for U24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_u32(), f)
    }
}

impl fmt::LowerHex for U24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.to_u32(), f)
    }
}

impl fmt::UpperHex for U24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.to_u32(), f)
    }
}

impl const Shl<u8> for U24 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        Self::from_u32(self.to_u32().shl(rhs))
    }
}

impl const Shr<u8> for U24 {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        Self::from_u32(self.to_u32().shr(rhs))
    }
}

impl const Shl<u8> for &U24 {
    type Output = U24;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        U24::from_u32(self.to_u32().shl(rhs))
    }
}

impl const Shr<u8> for &U24 {
    type Output = U24;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        U24::from_u32(self.to_u32().shr(rhs))
    }
}

impl From<U24> for u32 {
    #[inline(always)]
    fn from(v: U24) -> Self {
        v.to_u32()
    }
}

impl From<U24> for u64 {
    #[inline(always)]
    fn from(v: U24) -> Self {
        u64::from(v.to_u32())
    }
}

impl U24 {
    /// Create a new `U24` from an unsigned 32-bit integer.
    ///
    /// The input value is truncated to 24 bits, providing larger value panics in a debug build.
    #[inline(always)]
    pub const fn from_u32(v: u32) -> Self {
        let b = v.to_le_bytes();
        debug_assert!(
            (v << u8::BITS) >> u8::BITS == v,
            "Input value exceeds 24 bits"
        );
        Self([b[0], b[1], b[2]])
    }

    /// Convert to an unsigned 32-bit integer
    #[inline(always)]
    pub const fn to_u32(self) -> u32 {
        let [a, b, c] = self.0;
        u32::from_le_bytes([a, b, c, 0])
    }
}

/// New type for signed integers that stores 24-bit numbers
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct I24([u8; 3]);

impl fmt::Debug for I24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.to_i32(), f)
    }
}

impl fmt::Display for I24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_i32(), f)
    }
}

impl fmt::LowerHex for I24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.to_i32(), f)
    }
}

impl fmt::UpperHex for I24 {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.to_i32(), f)
    }
}

impl const Shl<u8> for I24 {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        Self::from_i32(self.to_i32().shl(rhs))
    }
}

impl const Shr<u8> for I24 {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        Self::from_i32(self.to_i32().shr(rhs))
    }
}

impl const Shl<u8> for &I24 {
    type Output = I24;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        I24::from_i32(self.to_i32().shl(rhs))
    }
}

impl const Shr<u8> for &I24 {
    type Output = I24;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        I24::from_i32(self.to_i32().shr(rhs))
    }
}

impl From<I24> for i32 {
    #[inline(always)]
    fn from(v: I24) -> Self {
        v.to_i32()
    }
}

impl From<I24> for i64 {
    #[inline(always)]
    fn from(v: I24) -> Self {
        i64::from(v.to_i32())
    }
}

impl I24 {
    /// Create a new `I24` from a signed 32-bit integer.
    ///
    /// The input value is truncated to 24 bits, providing larger value panics in a debug build.
    #[inline(always)]
    pub const fn from_i32(v: i32) -> Self {
        let b = v.to_le_bytes();
        debug_assert!(
            (v << u8::BITS) >> u8::BITS == v,
            "Input value exceeds 24 bits"
        );
        Self([b[0], b[1], b[2]])
    }

    /// Convert to a signed 32-bit integer
    #[inline(always)]
    pub const fn to_i32(self) -> i32 {
        let [a, b, c] = self.0;
        // Sign-extend
        i32::from_le_bytes([a, b, c, 0]) << u8::BITS >> u8::BITS
    }
}

/// New type for signed integers that stores 32-bit numbers with `LOW_ZEROED_BITS` low bits zeroed
/// and truncated to 24-bits
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct I24WithZeroedBits<const LOW_ZEROED_BITS: u8>([u8; 3]);

impl<const LOW_ZEROED_BITS: u8> fmt::Debug for I24WithZeroedBits<LOW_ZEROED_BITS> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.to_i32(), f)
    }
}

impl<const LOW_ZEROED_BITS: u8> fmt::Display for I24WithZeroedBits<LOW_ZEROED_BITS> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_i32(), f)
    }
}

impl<const LOW_ZEROED_BITS: u8> fmt::LowerHex for I24WithZeroedBits<LOW_ZEROED_BITS> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.to_i32(), f)
    }
}

impl<const LOW_ZEROED_BITS: u8> fmt::UpperHex for I24WithZeroedBits<LOW_ZEROED_BITS> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.to_i32(), f)
    }
}

impl<const LOW_ZEROED_BITS: u8> const Shl<u8> for I24WithZeroedBits<LOW_ZEROED_BITS> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        Self::from_i32(self.to_i32().shl(rhs))
    }
}

impl<const LOW_ZEROED_BITS: u8> const Shr<u8> for I24WithZeroedBits<LOW_ZEROED_BITS> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        Self::from_i32(self.to_i32().shr(rhs))
    }
}

impl<const LOW_ZEROED_BITS: u8> const Shl<u8> for &I24WithZeroedBits<LOW_ZEROED_BITS> {
    type Output = I24WithZeroedBits<LOW_ZEROED_BITS>;

    #[inline(always)]
    fn shl(self, rhs: u8) -> Self::Output {
        I24WithZeroedBits::from_i32(self.to_i32().shl(rhs))
    }
}

impl<const LOW_ZEROED_BITS: u8> const Shr<u8> for &I24WithZeroedBits<LOW_ZEROED_BITS> {
    type Output = I24WithZeroedBits<LOW_ZEROED_BITS>;

    #[inline(always)]
    fn shr(self, rhs: u8) -> Self::Output {
        I24WithZeroedBits::from_i32(self.to_i32().shr(rhs))
    }
}

impl<const LOW_ZEROED_BITS: u8> From<I24WithZeroedBits<LOW_ZEROED_BITS>> for i32 {
    #[inline(always)]
    fn from(v: I24WithZeroedBits<LOW_ZEROED_BITS>) -> Self {
        v.to_i32()
    }
}

impl<const LOW_ZEROED_BITS: u8> From<I24WithZeroedBits<LOW_ZEROED_BITS>> for i64 {
    #[inline(always)]
    fn from(v: I24WithZeroedBits<LOW_ZEROED_BITS>) -> Self {
        i64::from(v.to_i32())
    }
}

impl<const LOW_ZEROED_BITS: u8> I24WithZeroedBits<LOW_ZEROED_BITS> {
    /// Create a new `I24WithZeroedBits` from a signed 32-bit integer.
    ///
    /// The input value is shifted right arithmetically by `LOW_ZEROED_BITS` before being stored.
    /// When converted back with [`Self::to_i32`], the value is shifted back with low bits being
    /// zero.
    #[inline(always)]
    pub const fn from_i32(v_original: i32) -> Self {
        let v = v_original >> LOW_ZEROED_BITS;
        let b = v.to_le_bytes();
        let return_value = Self([b[0], b[1], b[2]]);

        debug_assert!(
            return_value.to_i32() == v_original,
            "Input has non-zero low bits"
        );

        return_value
    }

    /// Convert to a signed 32-bit integer
    #[inline(always)]
    pub const fn to_i32(self) -> i32 {
        let [a, b, c] = self.0;
        // Sign-extend and shift back
        (((i32::from_le_bytes([a, b, c, 0]) << u8::BITS >> u8::BITS) << LOW_ZEROED_BITS)
            .cast_unsigned()
            & (u32::MAX << LOW_ZEROED_BITS))
            .cast_signed()
    }
}
