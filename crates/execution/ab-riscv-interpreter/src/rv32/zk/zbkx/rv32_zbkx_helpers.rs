//! Opaque helpers for Zbkx extension

#[inline]
#[doc(hidden)]
pub fn xperm4(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkx") => {
            unsafe { core::arch::riscv32::xperm4(rs1 as usize, rs2 as usize) as u32 }
        }
        _ => {
            use core::simd::num::SimdUint;
            use core::simd::u32x8;

            const SHIFT: u32x8 = u32x8::from_array([0, 4, 8, 12, 16, 20, 24, 28]);
            const MASK: u32x8 = u32x8::splat(0xf);

            // Unpack nibbles of rs1 into bytes via SIMD: broadcast, shift per-lane, mask
            let lut = (u32x8::splat(rs1) >> SHIFT) & MASK;
            // Unpack nibbles of rs2 into byte indices via SIMD
            let idx = (u32x8::splat(rs2) >> SHIFT) & MASK;
            // For each nibble of rs2, look up from lut (out-of-bounds -> 0 via swizzle_dyn)
            let nibbles = lut.cast().swizzle_dyn(idx.cast());
            // TODO: Including modified macro expansions for now, switch back to the macro once
            //  migrated to the newer nightly
            // Pack nibbles back: interleave even/odd lanes and fold into bytes
            // let lo = simd_swizzle!(nibbles, [0, 2, 4, 6]);
            let lo = {
                use core::simd::Swizzle;
                struct Impl;
                impl Swizzle<const { [0, 2, 4, 6].len() }> for Impl {
                    const INDEX: [usize; const { [0, 2, 4, 6].len() }] = [0, 2, 4, 6];
                }
                Impl::swizzle(nibbles)
            };
            // let hi = simd_swizzle!(nibbles, [1, 3, 5, 7]);
            let hi = {
                use core::simd::Swizzle;
                struct Impl;
                impl Swizzle<const { [1, 3, 5, 7].len() }> for Impl {
                    const INDEX: [usize; const { [1, 3, 5, 7].len() }] = [1, 3, 5, 7];
                }
                Impl::swizzle(nibbles)
            };
            u32::from_le_bytes((lo | (hi << 4)).to_array())
        }
    }
}

#[inline]
#[doc(hidden)]
pub fn xperm8(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkx") => {
            unsafe { core::arch::riscv32::xperm8(rs1 as usize, rs2 as usize) as u32 }
        }
        _ => {
            use core::simd::u8x4;

            let lut = u8x4::from_array(rs1.to_le_bytes());
            let idx = u8x4::from_array(rs2.to_le_bytes());

            let result = lut.swizzle_dyn(idx);

            u32::from_le_bytes(result.to_array())
        }
    }
}
