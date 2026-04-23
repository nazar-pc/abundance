//! Opaque helpers for Zbb extension

#[inline(always)]
#[doc(hidden)]
pub fn orc_b(src: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbb") => {
            // SAFETY: Compile-time checked for supported feature
            unsafe { core::arch::riscv32::orc_b(src as usize) as u32 }
        }
        _ => {{
            let mut bytes = src.to_le_bytes();
            for byte in &mut bytes {
                *byte = if *byte != 0 { 0xFF } else { 0 };
            }
            u32::from_le_bytes(bytes)
        }}
    }
}
