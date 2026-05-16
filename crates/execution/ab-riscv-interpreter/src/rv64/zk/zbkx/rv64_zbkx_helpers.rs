//! Opaque helpers for Zbkx extension

#[inline]
#[doc(hidden)]
pub fn xperm4(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm4(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            use core::simd::num::SimdUint;
            use core::simd::{simd_swizzle, u64x16};

            const SHIFT: u64x16 =
                u64x16::from_array([0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60]);
            const MASK: u64x16 = u64x16::splat(0xf);

            // Unpack nibbles of rs1 into bytes via SIMD: broadcast, shift per-lane, mask
            let lut = (u64x16::splat(rs1) >> SHIFT) & MASK;
            // Unpack nibbles of rs2 into byte indices via SIMD
            let idx = (u64x16::splat(rs2) >> SHIFT) & MASK;
            // For each nibble of rs2, look up directly from lut; all indices 0–15 are in-bounds
            let nibbles = lut.cast().swizzle_dyn(idx.cast());
            // Pack nibbles back: interleave even/odd lanes and fold into bytes
            let lo = simd_swizzle!(nibbles, [0, 2, 4, 6, 8, 10, 12, 14]);
            let hi = simd_swizzle!(nibbles, [1, 3, 5, 7, 9, 11, 13, 15]);
            u64::from_le_bytes((lo | (hi << 4)).to_array())
        }
    }
}

#[inline]
#[doc(hidden)]
pub fn xperm8(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm8(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            use core::simd::u8x8;

            let lut = u8x8::from_array(rs1.to_le_bytes());
            let idx = u8x8::from_array(rs2.to_le_bytes());

            let result = lut.swizzle_dyn(idx);

            u64::from_le_bytes(result.to_array())
        }
    }
}
