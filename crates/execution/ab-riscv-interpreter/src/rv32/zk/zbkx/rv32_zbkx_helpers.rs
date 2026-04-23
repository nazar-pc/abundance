//! Opaque helpers for Zbkx extension

#[inline(always)]
#[doc(hidden)]
pub fn xperm4(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkx") => {
            unsafe { core::arch::riscv32::xperm4(rs1 as usize, rs2 as usize) as u32 }
        }
        _ => {
            // Build nibble LUT from rs1: 8 entries for RV32
            let mut lut = [0; 8];
            for (i, l) in lut.iter_mut().enumerate() {
                *l = ((rs1 >> (i * 4)) & 0xf) as u8;
            }
            // For each nibble of rs2, look up from lut (out-of-bounds -> 0 via get)
            let mut result = 0;
            for i in 0..8 {
                let idx = ((rs2 >> (i * 4)) & 0xf) as usize;
                result |= u32::from(*lut.get(idx).unwrap_or(&0)) << (i * 4);
            }
            result
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn xperm8(rs1: u32, rs2: u32) -> u32 {
    // TODO: Miri is excluded because corresponding intrinsic is not implemented there
    cfg_select! {
        all(not(miri), target_arch = "riscv32", target_feature = "zbkx") => {
            unsafe { core::arch::riscv32::xperm8(rs1 as usize, rs2 as usize) as u32 }
        }
        _ => {
            let lut = rs1.to_le_bytes();
            let mut result = [0; _];
            // Explicit loop to ensure inlining
            for (&idx, r) in rs2.to_le_bytes().iter().zip(&mut result) {
                *r = *lut.get(usize::from(idx)).unwrap_or(&0);
            }
            u32::from_le_bytes(result)
        }
    }
}
