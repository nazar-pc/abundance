//! Opaque helpers for RV32 Zbc extension

#[inline(always)]
#[doc(hidden)]
pub fn clmul(a: u32, b: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::clmul(a as usize, b as usize) as u32 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            result as u32
        }}
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn clmulh(a: u32, b: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::clmulh(a as usize, b as usize) as u32 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            (result >> 32) as u32
        }}
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn clmulr(a: u32, b: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::clmulr(a as usize, b as usize) as u32 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            (result >> 31) as u32
        }}
    }
}

/// Carryless multiplication helper
#[cfg(any(miri, not(all(target_arch = "riscv32", target_feature = "zbc"))))]
#[inline(always)]
#[doc(hidden)]
fn clmul_internal(a: u32, b: u32) -> u64 {
    let a = u64::from(a);
    let b = u64::from(b);

    cfg_select! {
        // TODO: `llvm.aarch64.neon.pmull64` is not supported in Miri yet:
        //  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
        all(
            not(miri), target_arch = "aarch64", target_feature = "neon", target_feature = "aes"
        ) => {{
            use core::arch::aarch64::vmull_p64;

            // SAFETY: Necessary target features enabled
            // Only lower 32 bits of a and b are meaningful; result fits in 64 bits
            unsafe { vmull_p64(a, b) as u64 }
        }}
        all(target_arch = "x86_64", target_feature = "pclmulqdq") => {{
            use core::arch::x86_64::{__m128i, _mm_clmulepi64_si128, _mm_cvtsi64_si128};
            use core::mem::transmute;

            // SAFETY: Necessary target features enabled
            unsafe {
                let result = transmute::<__m128i, u128>(_mm_clmulepi64_si128(
                    _mm_cvtsi64_si128(a.cast_signed()),
                    _mm_cvtsi64_si128(b.cast_signed()),
                    0,
                ));
                result as u64
            }
        }}
        _ => {{
            // Generic implementation: inputs are at most 32 bits wide, result fits in 64 bits
            let mut result = 0u64;
            let mut b = b;
            for i in 0..u32::BITS {
                let bit = b & 1;
                result ^= a.wrapping_shl(i) & (0u64.wrapping_sub(bit));
                b >>= 1;
            }
            result
        }}
    }
}
