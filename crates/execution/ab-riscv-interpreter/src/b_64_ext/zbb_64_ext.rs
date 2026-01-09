//! RV64 Zbb extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::b_64_ext::zbb_64_ext::Zbb64ExtInstruction;
use ab_riscv_primitives::registers::{Register, Registers};

/// Execute instructions from Zbb extension
#[inline(always)]
pub fn execute_zbb_64_ext<Reg>(regs: &mut Registers<Reg>, instruction: Zbb64ExtInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Zbb64ExtInstruction::Andn { rd, rs1, rs2 } => {
            let value = regs.read(rs1) & !regs.read(rs2);
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Orn { rd, rs1, rs2 } => {
            let value = regs.read(rs1) | !regs.read(rs2);
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Xnor { rd, rs1, rs2 } => {
            let value = !(regs.read(rs1) ^ regs.read(rs2));
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Clz { rd, rs1 } => {
            let value = regs.read(rs1).leading_zeros() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Clzw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).leading_zeros() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Ctz { rd, rs1 } => {
            let value = regs.read(rs1).trailing_zeros() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Ctzw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).trailing_zeros() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Cpop { rd, rs1 } => {
            let value = regs.read(rs1).count_ones() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Cpopw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).count_ones() as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Max { rd, rs1, rs2 } => {
            let a = regs.read(rs1).cast_signed();
            let b = regs.read(rs2).cast_signed();
            let value = a.max(b).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Maxu { rd, rs1, rs2 } => {
            let value = regs.read(rs1).max(regs.read(rs2));
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Min { rd, rs1, rs2 } => {
            let a = regs.read(rs1).cast_signed();
            let b = regs.read(rs2).cast_signed();
            let value = a.min(b).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Minu { rd, rs1, rs2 } => {
            let value = regs.read(rs1).min(regs.read(rs2));
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Sextb { rd, rs1 } => {
            let value = ((regs.read(rs1) as i8) as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Sexth { rd, rs1 } => {
            let value = ((regs.read(rs1) as i16) as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Zexth { rd, rs1 } => {
            let value = (regs.read(rs1) as u16) as u64;
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Rol { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x3f) as u32;
            let value = regs.read(rs1).rotate_left(shamt);
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Rolw { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x1f) as u32;
            let value =
                ((regs.read(rs1) as u32).rotate_left(shamt).cast_signed() as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Ror { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x3f) as u32;
            let value = regs.read(rs1).rotate_right(shamt);
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Rori { rd, rs1, shamt } => {
            let value = regs.read(rs1).rotate_right((shamt & 0x3f) as u32);
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Roriw { rd, rs1, shamt } => {
            let value = ((regs.read(rs1) as u32)
                .rotate_right((shamt & 0x1f) as u32)
                .cast_signed() as i64)
                .cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Rorw { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x1f) as u32;
            let value =
                ((regs.read(rs1) as u32).rotate_right(shamt).cast_signed() as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Zbb64ExtInstruction::Orcb { rd, rs1 } => {
            let src = regs.read(rs1);
            let mut result = 0u64;
            for i in 0..8 {
                let byte = (src >> (i * 8)) & 0xFF;
                if byte != 0 {
                    result |= 0xFFu64 << (i * 8);
                }
            }
            regs.write(rd, result);
        }
        Zbb64ExtInstruction::Rev8 { rd, rs1 } => {
            let value = regs.read(rs1).swap_bytes();
            regs.write(rd, value);
        }
    }
}
