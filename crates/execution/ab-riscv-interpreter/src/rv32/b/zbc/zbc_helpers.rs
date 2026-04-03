//! Opaque helpers for RV32 Zbc extension

/// Carryless multiplication helper
#[cfg(any(miri, not(all(target_arch = "riscv32", target_feature = "zbc"))))]
#[inline(always)]
#[doc(hidden)]
pub fn clmul_internal(a: u32, b: u32) -> u64 {
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

/// Only here to prevent compiler warnings about unused `zbc_helpers` module
#[doc(hidden)]
pub const PLACEHOLDER: () = ();
