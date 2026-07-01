//! Opaque helpers for RV32 Zbkb extension

#[inline(always)]
#[doc(hidden)]
pub fn zip(src: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::zip(src) }
        }
        _ => {{
            // Spread each 16-bit half into alternating bits.
            // Classic SWAR interleave for 16-bit -> 32-bit Morton.
            #[inline(always)]
            fn spread(mut x: u32) -> u32 {
                x = (x | (x << 8u8)) & 0x00FF_00FF;
                x = (x | (x << 4u8)) & 0x0F0F_0F0F;
                x = (x | (x << 2u8)) & 0x3333_3333;
                (x | (x << 1u8)) & 0x5555_5555
            }

            let lo = src & 0x0000_FFFF;
            let hi = src >> 16u8;

            spread(lo) | (spread(hi) << 1u8)
        }}
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn unzip(src: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::unzip(src) }
        }
        _ => {{
            #[inline(always)]
            fn compact(mut x: u32) -> u32 {
                x &= 0x5555_5555;
                x = (x | (x >> 1u8)) & 0x3333_3333;
                x = (x | (x >> 2u8)) & 0x0F0F_0F0F;
                x = (x | (x >> 4u8)) & 0x00FF_00FF;
                (x | (x >> 8u8)) & 0x0000_FFFF
            }

            let lo = compact(src);
            let hi = compact(src >> 1u8);
            lo | (hi << 16u8)
        }}
    }
}
