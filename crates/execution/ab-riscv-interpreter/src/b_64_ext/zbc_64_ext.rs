//! RV64 Zbc extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::b_64_ext::zbc_64_ext::Zbc64ExtInstruction;
use ab_riscv_primitives::registers::{GenericRegister64, GenericRegisters64};

/// Carryless multiplication helper
#[inline]
fn clmul_internal(a: u64, b: u64) -> u128 {
    let mut result = 0u128;
    for i in 0..64 {
        if (b >> i) & 1 != 0 {
            result ^= (a as u128) << i;
        }
    }
    result
}

/// Execute instructions from Zbc extension
#[inline(always)]
pub fn execute_zbc_64_ext<Reg, Registers>(
    regs: &mut Registers,
    instruction: Zbc64ExtInstruction<Reg>,
) where
    Reg: GenericRegister64,
    Registers: GenericRegisters64<Reg>,
{
    match instruction {
        Zbc64ExtInstruction::Clmul { rd, rs1, rs2 } => {
            let result = clmul_internal(regs.read(rs1), regs.read(rs2));
            regs.write(rd, result as u64);
        }
        Zbc64ExtInstruction::Clmulh { rd, rs1, rs2 } => {
            let result = clmul_internal(regs.read(rs1), regs.read(rs2));
            regs.write(rd, (result >> 64) as u64);
        }
        Zbc64ExtInstruction::Clmulr { rd, rs1, rs2 } => {
            let result = clmul_internal(regs.read(rs1), regs.read(rs2));
            regs.write(rd, (result >> 1) as u64);
        }
    }
}
