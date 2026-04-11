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
            let bytes = src.to_le_bytes().map(|b| if b != 0 { 0xFFu8 } else { 0u8 });
            u64::from_le_bytes(bytes)
        }}
    }
}
