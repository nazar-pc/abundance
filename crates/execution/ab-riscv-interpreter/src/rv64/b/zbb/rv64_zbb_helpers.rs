//! Opaque helpers for Zbb extension

#[inline(always)]
#[doc(hidden)]
pub fn orc_b(src: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv64::orc_b(src as usize) as u64 }
        }
        _ => {{
            let mut bytes = src.to_le_bytes();
            // Explicit loop to ensure inlining
            for byte in &mut bytes {
                *byte = if *byte != 0 { 0xFF } else { 0 };
            }
            u64::from_le_bytes(bytes)
        }}
    }
}
