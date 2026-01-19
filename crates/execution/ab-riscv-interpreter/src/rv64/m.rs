//! RV64 M extension

#[cfg(test)]
mod tests;

use crate::ExecutionError;
use crate::rv64::{ExecutableInstruction, Rv64InterpreterState};
use ab_riscv_primitives::instruction::rv64::m::Rv64MInstruction;
use ab_riscv_primitives::registers::Register;
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
        match self {
            Self::Mul { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_mul(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Mulh { rd, rs1, rs2 } => {
                let (_lo, prod) = state
                    .regs
                    .read(rs1)
                    .cast_signed()
                    .widening_mul(state.regs.read(rs2).cast_signed());
                state.regs.write(rd, prod.cast_unsigned());
            }
            Self::Mulhsu { rd, rs1, rs2 } => {
                let prod =
                    (state.regs.read(rs1).cast_signed() as i128) * (state.regs.read(rs2) as i128);
                let value = prod >> 64;
                state.regs.write(rd, value.cast_unsigned() as u64);
            }
            Self::Mulhu { rd, rs1, rs2 } => {
                let prod = (state.regs.read(rs1) as u128) * (state.regs.read(rs2) as u128);
                let value = prod >> 64;
                state.regs.write(rd, value as u64);
            }
            Self::Div { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1).cast_signed();
                let divisor = state.regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i64::MIN && divisor == -1 {
                    i64::MIN
                } else {
                    dividend / divisor
                };
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Divu { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1);
                let divisor = state.regs.read(rs2);
                let value = if divisor == 0 {
                    u64::MAX
                } else {
                    dividend / divisor
                };
                state.regs.write(rd, value);
            }
            Self::Rem { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1).cast_signed();
                let divisor = state.regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i64::MIN && divisor == -1 {
                    0
                } else {
                    dividend % divisor
                };
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Remu { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1);
                let divisor = state.regs.read(rs2);
                let value = if divisor == 0 {
                    dividend
                } else {
                    dividend % divisor
                };
                state.regs.write(rd, value);
            }

            // RV64 R-type W
            Self::Mulw { rd, rs1, rs2 } => {
                let prod = (state.regs.read(rs1) as i32).wrapping_mul(state.regs.read(rs2) as i32);
                state.regs.write(rd, (prod as i64).cast_unsigned());
            }
            Self::Divw { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1) as i32;
                let divisor = state.regs.read(rs2) as i32;
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i32::MIN && divisor == -1 {
                    i32::MIN as i64
                } else {
                    (dividend / divisor) as i64
                };
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Divuw { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1) as u32;
                let divisor = state.regs.read(rs2) as u32;
                let value = if divisor == 0 {
                    u64::MAX
                } else {
                    ((dividend / divisor).cast_signed() as i64).cast_unsigned()
                };
                state.regs.write(rd, value);
            }
            Self::Remw { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1) as i32;
                let divisor = state.regs.read(rs2) as i32;
                let value = if divisor == 0 {
                    (dividend as i64).cast_unsigned()
                } else if dividend == i32::MIN && divisor == -1 {
                    0
                } else {
                    ((dividend % divisor) as i64).cast_unsigned()
                };
                state.regs.write(rd, value);
            }
            Self::Remuw { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1) as u32;
                let divisor = state.regs.read(rs2) as u32;
                let value = if divisor == 0 {
                    dividend.cast_signed() as i64
                } else {
                    (dividend % divisor).cast_signed() as i64
                };
                state.regs.write(rd, value.cast_unsigned());
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
