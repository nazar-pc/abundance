//! Opaque helpers for Zbkx extension

#[inline(always)]
#[doc(hidden)]
pub fn xperm4(rs1: u64, rs2: u64) -> u64 {
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm4(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            // 16 nibbles for RV64; all indices 0–15 are in-bounds, so direct indexing is safe
            let lut = core::array::from_fn::<_, 16, _>(|i| ((rs1 >> (i * 4)) & 0xf) as u8);
            // For each nibble of rs2, look up directly from lut
            let nibbles = core::array::from_fn::<_, 16, _>(|i| {
                let idx = ((rs2 >> (i * 4)) & 0xf) as usize;
                lut[idx]
            });
            // Pack nibbles back into u64
            nibbles.iter().enumerate().fold(0, |acc, (i, &n)| acc | (u64::from(n) << (i * 4)))
        }
    }
}

#[inline(always)]
#[doc(hidden)]
pub fn xperm8(rs1: u64, rs2: u64) -> u64 {
    cfg_select! {
        all(not(miri), target_arch = "riscv64", target_feature = "zbkx") => {
            unsafe { core::arch::riscv64::xperm8(rs1 as usize, rs2 as usize) as u64 }
        }
        _ => {
            let lut = rs1.to_le_bytes();
            let result = rs2.to_le_bytes().map(|idx| {
                *lut.get(usize::from(idx)).unwrap_or(&0)
            });
            u64::from_le_bytes(result)
        }
    }
}
