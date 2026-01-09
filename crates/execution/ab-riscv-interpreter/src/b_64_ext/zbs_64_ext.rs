//! RV64 Zbs extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::b_64_ext::zbs_64_ext::Zbs64ExtInstruction;
use ab_riscv_primitives::registers::{Register, Registers};

/// Execute instructions from Zbs extension
#[inline(always)]
pub fn execute_zbs_64_ext<Reg>(regs: &mut Registers<Reg>, instruction: Zbs64ExtInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Zbs64ExtInstruction::Bset { rd, rs1, rs2 } => {
            // Only the bottom 6 bits for RV64
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) | (1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Bseti { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) | (1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Bclr { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) & !(1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Bclri { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) & !(1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Binv { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) ^ (1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Binvi { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) ^ (1u64 << index);
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Bext { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = (regs.read(rs1) >> index) & 1;
            regs.write(rd, result);
        }
        Zbs64ExtInstruction::Bexti { rd, rs1, shamt } => {
            let index = shamt;
            let result = (regs.read(rs1) >> index) & 1;
            regs.write(rd, result);
        }
    }
}
