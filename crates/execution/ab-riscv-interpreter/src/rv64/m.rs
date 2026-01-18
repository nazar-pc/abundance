//! RV64 M extension

#[cfg(test)]
mod tests;

use crate::ExecutionError;
use crate::rv64::{ExecutableInstruction, Rv64InterpreterState};
use ab_riscv_primitives::instruction::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::registers::{Register, Registers};
use core::ops::ControlFlow;

impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64MInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        execute_m(&mut state.regs, self);

        Ok(ControlFlow::Continue(()))
    }
}

/// Execute instructions from M extension
#[inline(always)]
pub fn execute_m<Reg>(regs: &mut Registers<Reg>, instruction: Rv64MInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64MInstruction::Mul { rd, rs1, rs2 } => {
            let value = regs.read(rs1).wrapping_mul(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64MInstruction::Mulh { rd, rs1, rs2 } => {
            let (_lo, prod) = regs
                .read(rs1)
                .cast_signed()
                .widening_mul(regs.read(rs2).cast_signed());
            regs.write(rd, prod.cast_unsigned());
        }
        Rv64MInstruction::Mulhsu { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1).cast_signed() as i128) * (regs.read(rs2) as i128);
            let value = prod >> 64;
            regs.write(rd, value.cast_unsigned() as u64);
        }
        Rv64MInstruction::Mulhu { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1) as u128) * (regs.read(rs2) as u128);
            let value = prod >> 64;
            regs.write(rd, value as u64);
        }
        Rv64MInstruction::Div { rd, rs1, rs2 } => {
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
        Rv64MInstruction::Divu { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1);
            let divisor = regs.read(rs2);
            let value = if divisor == 0 {
                u64::MAX
            } else {
                dividend / divisor
            };
            regs.write(rd, value);
        }
        Rv64MInstruction::Rem { rd, rs1, rs2 } => {
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
        Rv64MInstruction::Remu { rd, rs1, rs2 } => {
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
        Rv64MInstruction::Mulw { rd, rs1, rs2 } => {
            let prod = (regs.read(rs1) as i32).wrapping_mul(regs.read(rs2) as i32);
            regs.write(rd, (prod as i64).cast_unsigned());
        }
        Rv64MInstruction::Divw { rd, rs1, rs2 } => {
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
        Rv64MInstruction::Divuw { rd, rs1, rs2 } => {
            let dividend = regs.read(rs1) as u32;
            let divisor = regs.read(rs2) as u32;
            let value = if divisor == 0 {
                u64::MAX
            } else {
                ((dividend / divisor).cast_signed() as i64).cast_unsigned()
            };
            regs.write(rd, value);
        }
        Rv64MInstruction::Remw { rd, rs1, rs2 } => {
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
        Rv64MInstruction::Remuw { rd, rs1, rs2 } => {
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
