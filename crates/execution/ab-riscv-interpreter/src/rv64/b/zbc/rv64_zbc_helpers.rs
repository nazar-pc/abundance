//! Opaque helpers for Zbc extension

#[inline(always)]
#[doc(hidden)]
pub fn clmul(a: u64, b: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::clmul(a as usize, b as usize) as u64 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            result as u64
        }}
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn clmulh(a: u64, b: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::clmulh(a as usize, b as usize) as u64 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            (result >> 64) as u64
        }}
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn clmulr(a: u64, b: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbc") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::clmulr(a as usize, b as usize) as u64 }
        }
        _ => {{
            let result = clmul_internal(a, b);
            (result >> 63) as u64
        }}
    }
}

/// Carryless multiplication helper
#[cfg(any(miri, not(all(target_arch = "riscv64", target_feature = "zbc"))))]
#[inline(always)]
fn clmul_internal(a: u64, b: u64) -> u128 {
    cfg_select! {
        // TODO: `llvm.aarch64.neon.pmull64` is not supported in Miri yet:
        //  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
        all(
            not(miri), target_arch = "aarch64", target_feature = "neon", target_feature = "aes"
        ) => {{
            use core::arch::aarch64::vmull_p64;

            // SAFETY: Compile-time checked for supported feature
            unsafe { vmull_p64(a, b) }
        }}
        all(target_arch = "x86_64", target_feature = "pclmulqdq") => {{
            use core::arch::x86_64::{__m128i, _mm_clmulepi64_si128, _mm_cvtsi64_si128};
            use core::mem::transmute;

            // SAFETY: Necessary target features enabled, `__m128i` and `u128` have the same memory
            // layout
            unsafe {
                transmute::<__m128i, u128>(_mm_clmulepi64_si128(
                    _mm_cvtsi64_si128(a.cast_signed()),
                    _mm_cvtsi64_si128(b.cast_signed()),
                    0,
                ))
            }
        }}
        _ => {{
            // Generic implementation
            let mut result = 0u128;
            let a = a as u128;
            let mut b = b;
            for i in 0..u64::BITS {
                let bit = (b & 1) as u128;
                result ^= a.wrapping_shl(i) & (0u128.wrapping_sub(bit));
                b >>= 1;
            }
            result
        }}
    }
}
