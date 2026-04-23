//! Opaque helpers for Zbkx extension

#[inline(always)]
#[doc(hidden)]
pub fn xperm4(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm4(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            // 16 nibbles for RV64; all indices 0–15 are in-bounds, so direct indexing is safe
            let mut lut = [0; 16];
            for (i, l) in lut.iter_mut().enumerate() {
                *l = ((rs1 >> (i * 4)) & 0xf) as u8;
            }
            // For each nibble of rs2, look up directly from lut
            let mut result = 0;
            for i in 0..16 {
                let idx = ((rs2 >> (i * 4)) & 0xf) as usize;
                // Pack nibbles back into u64
                result |= u64::from(lut[idx]) << (i * 4);
            }
            result
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn xperm8(rs1: u64, rs2: u64) -> u64 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm8(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            let lut = rs1.to_le_bytes();
            let mut result = [0; _];
            // Explicit loop to ensure inlining
            for (&idx, r) in rs2.to_le_bytes().iter().zip(&mut result) {
                *r = *lut.get(usize::from(idx)).unwrap_or(&0);
            }
            u64::from_le_bytes(result)
        }
    }
}
