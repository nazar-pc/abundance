//! RV64 M extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::instruction::m_64_ext::M64ExtInstruction;
use ab_riscv_primitives::registers::{Register, Registers};

/// Execute instructions from M extension
#[inline(always)]
pub fn execute_m_64_ext<Reg>(regs: &mut Registers<Reg>, instruction: M64ExtInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        M64ExtInstruction::Mul { rd, rs1, rs2 } => {
            let value = regs.read(rs1).wrapping_mul(regs.read(rs2));
            regs.write(rd, value);
        }
        M64ExtInstruction::Mulh { rd, rs1, rs2 } => {
            let (_lo, prod) = regs
                .read(rs1)
                .cast_signed()
                .widening_mul(regs.read(rs2).cast_signed());
            regs.write(rd, prod.cast_unsigned());
        }
        M64ExtInstruction::Mulhsu { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1).cast_signed() as i128) * (regs.read(rs2) as i128);
            let value = prod >> 64;
            regs.write(rd, value.cast_unsigned() as u64);
        }
        M64ExtInstruction::Mulhu { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1) as u128) * (regs.read(rs2) as u128);
            let value = prod >> 64;
            regs.write(rd, value as u64);
        }
        M64ExtInstruction::Div { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1).cast_signed();
            let divisor = regs.read(rs2).cast_signed();
            let value = if divisor == 0 {
                -1i64
            } else if dividend == i64::MIN && divisor == -1 {
                i64::MIN
            } else {
                dividend / divisor
            };
            regs.write(rd, value.cast_unsigned());
        }
        M64ExtInstruction::Divu { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1);
            let divisor = regs.read(rs2);
            let value = if divisor == 0 {
                u64::MAX
            } else {
                dividend / divisor
            };
            regs.write(rd, value);
        }
        M64ExtInstruction::Rem { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1).cast_signed();
            let divisor = regs.read(rs2).cast_signed();
            let value = if divisor == 0 {
                dividend
            } else if dividend == i64::MIN && divisor == -1 {
                0
            } else {
                dividend % divisor
            };
            regs.write(rd, value.cast_unsigned());
        }
        M64ExtInstruction::Remu { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1);
            let divisor = regs.read(rs2);
            let value = if divisor == 0 {
                dividend
            } else {
                dividend % divisor
            };
            regs.write(rd, value);
        }

        // RV64 R-type W
        M64ExtInstruction::Mulw { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1) as i32).wrapping_mul(regs.read(rs2) as i32);
            regs.write(rd, (prod as i64).cast_unsigned());
        }
        M64ExtInstruction::Divw { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1) as i32;
            let divisor = regs.read(rs2) as i32;
            let value = if divisor == 0 {
                -1i64
            } else if dividend == i32::MIN && divisor == -1 {
                i32::MIN as i64
            } else {
                (dividend / divisor) as i64
            };
            regs.write(rd, value.cast_unsigned());
        }
        M64ExtInstruction::Divuw { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1) as u32;
            let divisor = regs.read(rs2) as u32;
            let value = if divisor == 0 {
                u64::MAX
            } else {
                ((dividend / divisor).cast_signed() as i64).cast_unsigned()
            };
            regs.write(rd, value);
        }
        M64ExtInstruction::Remw { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1) as i32;
            let divisor = regs.read(rs2) as i32;
            let value = if divisor == 0 {
                (dividend as i64).cast_unsigned()
            } else if dividend == i32::MIN && divisor == -1 {
                0
            } else {
                ((dividend % divisor) as i64).cast_unsigned()
            };
            regs.write(rd, value);
        }
        M64ExtInstruction::Remuw { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1) as u32;
            let divisor = regs.read(rs2) as u32;
            let value = if divisor == 0 {
                dividend.cast_signed() as i64
            } else {
                (dividend % divisor).cast_signed() as i64
            };
            regs.write(rd, value.cast_unsigned());
        }
    }
}
