//! RV64 Zbc extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::registers::{Register, Registers};

/// Carryless multiplication helper
#[cfg(any(miri, not(all(target_arch = "riscv64", target_feature = "zbc"))))]
#[inline(always)]
fn clmul_internal(a: u64, b: u64) -> u128 {
    // TODO: `llvm.aarch64.neon.pmull64` is not supported in Miri yet:
    //  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
    #[cfg(all(
        not(miri),
        target_arch = "aarch64",
        target_feature = "neon",
        target_feature = "aes"
    ))]
    {
        use core::arch::aarch64::vmull_p64;

        // SAFETY: Necessary target features enabled
        unsafe { vmull_p64(a, b) }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "pclmulqdq"))]
    {
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
    }

    #[cfg(not(any(
        all(
            not(miri),
            target_arch = "aarch64",
            target_feature = "neon",
            target_feature = "aes"
        ),
        all(target_arch = "x86_64", target_feature = "pclmulqdq")
    )))]
    {
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
    }
}

/// Execute instructions from Zbc extension
#[inline(always)]
pub fn execute_zbc<Reg>(regs: &mut Registers<Reg>, instruction: Rv64ZbcInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64ZbcInstruction::Clmul { rd, rs1, rs2 } => {
            let a = regs.read(rs1);
            let b = regs.read(rs2);

            #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zbc"))]
            let value = core::arch::riscv64::clmul(a as usize, b as usize) as u64;

            #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zbc")))]
            let value = {
                let result = clmul_internal(a, b);
                result as u64
            };

            regs.write(rd, value);
        }
        Rv64ZbcInstruction::Clmulh { rd, rs1, rs2 } => {
            let a = regs.read(rs1);
            let b = regs.read(rs2);

            #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zbc"))]
            let value = core::arch::riscv64::clmulh(a as usize, b as usize) as u64;

            #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zbc")))]
            let value = {
                let result = clmul_internal(a, b);
                (result >> 64) as u64
            };

            regs.write(rd, value);
        }
        Rv64ZbcInstruction::Clmulr { rd, rs1, rs2 } => {
            let a = regs.read(rs1);
            let b = regs.read(rs2);

            #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zbc"))]
            let value = core::arch::riscv64::clmulr(a as usize, b as usize) as u64;

            #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zbc")))]
            let value = {
                let result = clmul_internal(a, b);
                (result >> 1) as u64
            };

            regs.write(rd, value);
        }
    }
}
