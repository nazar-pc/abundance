//! Part of the interpreter responsible for RISC-V M extension

use ab_riscv_primitives::instruction::m_64_ext::M64ExtInstruction;
use ab_riscv_primitives::registers::{GenericRegister64, GenericRegisters64};

/// Execute instructions from M extension
#[inline(always)]
pub fn execute_m_64_ext<Reg, Registers>(regs: &mut Registers, instruction: M64ExtInstruction<Reg>)
where
    Reg: GenericRegister64,
    Registers: GenericRegisters64<Reg>,
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
    }
}
