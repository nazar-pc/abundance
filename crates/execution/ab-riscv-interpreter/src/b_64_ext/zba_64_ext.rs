//! RV64 Zba extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::b_64_ext::zba_64_ext::Zba64ExtInstruction;
use ab_riscv_primitives::registers::{Register, Registers};

/// Execute instructions from Zba extension
#[inline(always)]
pub fn execute_zba_64_ext<Reg>(regs: &mut Registers<Reg>, instruction: Zba64ExtInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Zba64ExtInstruction::AddUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = rs1_val.wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh1add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 1).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh1addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 1).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh2add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 2).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh2addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 2).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh3add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 3).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::Sh3addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 3).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Zba64ExtInstruction::SlliUw { rd, rs1, shamt } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = rs1_val << (shamt & 0x3f);
            regs.write(rd, value);
        }
    }
}
