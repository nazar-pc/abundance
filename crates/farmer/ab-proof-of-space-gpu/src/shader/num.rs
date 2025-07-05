mod native;
mod u32;
mod u64;

#[cfg(not(target_arch = "spirv"))]
pub(super) use crate::shader::num::native::U64;
#[cfg(not(target_arch = "spirv"))]
#[expect(unused_imports, reason = "Not using U128 yet")]
pub(super) use crate::shader::num::native::U128;
#[cfg(all(target_arch = "spirv", not(target_feature = "Int64")))]
pub(super) use crate::shader::num::u32::U64;
#[cfg(all(target_arch = "spirv", not(target_feature = "Int64")))]
pub(super) use crate::shader::num::u32::U128;
#[cfg(all(target_arch = "spirv", target_feature = "Int64"))]
#[expect(unused_imports, reason = "Not using U128 yet")]
pub(super) use crate::shader::num::u64::U64;
#[cfg(all(target_arch = "spirv", target_feature = "Int64"))]
#[expect(unused_imports, reason = "Not using U128 yet")]
pub(super) use crate::shader::num::u64::U128;
use core::cmp::{Eq, PartialEq};
use core::fmt;
use core::hash::Hash;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Shl, ShlAssign,
    Shr, ShrAssign, Sub, SubAssign,
};

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
pub(super) trait U64T:
    fmt::Debug
    + Copy
    + Clone
    + Eq
    + PartialEq
    + Hash
    + From<u32>
    + Add
    + AddAssign
    + Sub
    + SubAssign
    + BitAnd
    + BitAndAssign
    + BitXor
    + BitXorAssign
    + BitOr
    + BitOrAssign
    + Shl<u32>
    + ShlAssign<u32>
    + Shr<u32>
    + ShrAssign<u32>
{
    fn from_lo_hi(lo: u32, hi: u32) -> Self;

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn to_be_bytes(self) -> [u8; 8];

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn from_be_bytes(bytes: [u8; 8]) -> Self;

    fn as_u32(self) -> u32;
}

// TODO: Remove once https://github.com/Rust-GPU/rust-gpu/discussions/301 has a better solution
pub(super) trait U128T:
    fmt::Debug
    + Copy
    + Clone
    + Eq
    + PartialEq
    + Hash
    + From<u32>
    + Add
    + AddAssign
    + Sub
    + SubAssign
    + BitAnd
    + BitAndAssign
    + BitXor
    + BitXorAssign
    + BitOr
    + BitOrAssign
    + Shl<u32>
    + ShlAssign<u32>
    + Shr<u32>
    + ShrAssign<u32>
{
    #[expect(dead_code, reason = "Not used yet")]
    const ZERO: Self;

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn to_be_bytes(self) -> [u8; 16];

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn from_be_bytes(bytes: [u8; 16]) -> Self;

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn as_be_bytes_to_le_u32_words(&self) -> [u32; 4];

    #[cfg_attr(not(test), expect(dead_code, reason = "Not used yet"))]
    fn from_le_u32_words_as_be_bytes(words: &[u32; 4]) -> Self;
}
