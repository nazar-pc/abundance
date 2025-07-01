#[cfg(test)]
mod tests;

use crate::shader::num::{U64T, U128T};
use core::mem;

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

    #[inline(always)]
    fn as_be_bytes_to_le_u32_words(&self) -> [u32; 4] {
        // SAFETY: All bit patterns are valid, output alignment is lower than input
        let be_words = unsafe { mem::transmute::<&u128, &[u32; 4]>(self) };

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

        // SAFETY: All bit patterns are valid
        unsafe { be_words.as_ptr().cast::<u128>().read_unaligned() }
    }
}
