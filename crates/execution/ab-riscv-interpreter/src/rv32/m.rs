//! RV32 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv32::m::Rv32MInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32MInstruction<Reg>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Mul { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_mul(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Mulh { rd, rs1, rs2 } => {
                // Signed × signed: widen to i64, take upper 32 bits
                let (_lo, prod) = state
                    .regs
                    .read(rs1)
                    .cast_signed()
                    .widening_mul(state.regs.read(rs2).cast_signed());
                state.regs.write(rd, prod.cast_unsigned());
            }
            Self::Mulhsu { rd, rs1, rs2 } => {
                // Signed × unsigned: widen to i64, take upper 32 bits
                let prod =
                    i64::from(state.regs.read(rs1).cast_signed()) * i64::from(state.regs.read(rs2));
                let value = prod >> 32;
                state.regs.write(rd, value.cast_unsigned() as u32);
            }
            Self::Mulhu { rd, rs1, rs2 } => {
                // Unsigned × unsigned: widen to u64, take upper 32 bits
                let prod = u64::from(state.regs.read(rs1)) * u64::from(state.regs.read(rs2));
                let value = prod >> 32;
                state.regs.write(rd, value as u32);
            }
            Self::Div { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1).cast_signed();
                let divisor = state.regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    -1i32
                } else if dividend == i32::MIN && divisor == -1 {
                    i32::MIN
                } else {
                    dividend / divisor
                };
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Divu { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1);
                let divisor = state.regs.read(rs2);
                let value = dividend.checked_div(divisor).unwrap_or(u32::MAX);
                state.regs.write(rd, value);
            }
            Self::Rem { rd, rs1, rs2 } => {
                let dividend = state.regs.read(rs1).cast_signed();
                let divisor = state.regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i32::MIN && divisor == -1 {
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
        }

        Ok(ControlFlow::Continue(()))
    }
}
