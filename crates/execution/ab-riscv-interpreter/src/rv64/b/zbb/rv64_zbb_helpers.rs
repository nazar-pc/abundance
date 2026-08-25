//! Opaque helpers for Zbb extension

use const_fn_specialization::const_fn_specialization;

#[const_fn_specialization]
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn orc_b(src: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::orc_b(src as usize) as u64 }
        }
        _ => orc_b_generic(src),
    }
}

#[const_fn_specialization]
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub const fn orc_b(src: u64) -> u64 {
    orc_b_generic(src)
}

#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
const fn orc_b_generic(src: u64) -> u64 {
    let bytes = src.to_le_bytes();

    u64::from_le_bytes([
        if bytes[0] != 0 { 0xFF } else { 0 },
        if bytes[1] != 0 { 0xFF } else { 0 },
        if bytes[2] != 0 { 0xFF } else { 0 },
        if bytes[3] != 0 { 0xFF } else { 0 },
        if bytes[4] != 0 { 0xFF } else { 0 },
        if bytes[5] != 0 { 0xFF } else { 0 },
        if bytes[6] != 0 { 0xFF } else { 0 },
        if bytes[7] != 0 { 0xFF } else { 0 },
    ])
}
