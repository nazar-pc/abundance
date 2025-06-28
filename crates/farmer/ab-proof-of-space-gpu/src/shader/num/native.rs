#[cfg(test)]
mod tests;

use crate::shader::num::{U64T, U128T};

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
pub(in super::super) type U64 = u64;

impl U64T for U64 {
    #[inline(always)]
    fn from_lo_hi(lo: u32, hi: u32) -> Self {
        (u64::from(hi) << u32::BITS) | u64::from(lo)
    }

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; 8] {
        self.to_be_bytes()
    }

    #[inline(always)]
    fn from_be_bytes(bytes: [u8; 8]) -> Self {
        u64::from_be_bytes(bytes)
    }

    #[inline(always)]
    fn from_u32(n: u32) -> Self {
        n as u64
    }

    #[inline(always)]
    fn as_u32(self) -> u32 {
        self as u32
    }
}

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
pub(in super::super) type U128 = u128;

impl U128T for U128 {
    #[inline(always)]
    fn to_be_bytes(self) -> [u8; 16] {
        self.to_be_bytes()
    }

    #[inline(always)]
    fn from_be_bytes(bytes: [u8; 16]) -> Self {
        u128::from_be_bytes(bytes)
    }
}
